use std::path::PathBuf;
use std::time::Duration;

use axum::{Router, routing::get};
use clap::Parser;
use ersha_core::{BatchId, BatchUploadRequest, DispatcherId, H3Cell, HelloRequest};
use ersha_dispatch::edge::tcp::TcpEdgeReceiver;
use ersha_dispatch::{
    Config, DeviceStatusStorage, EdgeConfig, EdgeData, EdgeReceiver, MemoryStorage,
    MockEdgeReceiver, SensorReadingsStorage, SqliteStorage, StorageConfig,
};
use ersha_rpc::Client;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use ulid::Ulid;

#[derive(Parser)]
#[command(name = "ersha-dispatch")]
#[command(about = "Ersha Dispatch")]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, default_value = "ersha-dispatch.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "tracing=info,ersha_dispatch=info".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let cli = Cli::parse();

    let config = if cli.config.exists() {
        info!(path = ?cli.config, "Loading configuration");
        Config::load(&cli.config)?
    } else {
        info!("No configuration file found, using defaults");
        Config::default()
    };

    let dispatcher_id: DispatcherId = DispatcherId(config.dispatcher.id.parse().map_err(|e| {
        color_eyre::eyre::eyre!("invalid dispatcher ID '{}': {}", config.dispatcher.id, e)
    })?);
    let location = H3Cell(config.dispatcher.location);

    info!(
        dispatcher_id = ?dispatcher_id,
        location = ?location,
        http_addr = %config.server.http_addr,
        prime_addr = %config.prime.rpc_addr,
        "Starting ersha-dispatch"
    );

    match config.storage {
        StorageConfig::Memory => {
            info!("Using in-memory storage");
            let storage = MemoryStorage::default();
            run_dispatcher(config, storage, dispatcher_id, location).await?;
        }
        StorageConfig::Sqlite { ref path } => {
            info!(path = ?path, "Using SQLite storage");
            let storage = SqliteStorage::new(path).await?;
            run_dispatcher(config, storage, dispatcher_id, location).await?;
        }
    }

    Ok(())
}

async fn run_dispatcher<S>(
    config: Config,
    storage: S,
    dispatcher_id: DispatcherId,
    location: H3Cell,
) -> color_eyre::Result<()>
where
    S: SensorReadingsStorage + DeviceStatusStorage + Clone + Send + Sync + 'static,
    <S as SensorReadingsStorage>::Error: std::error::Error + Send + Sync + 'static,
    <S as DeviceStatusStorage>::Error: std::error::Error + Send + Sync + 'static,
{
    let cancel = CancellationToken::new();

    // Create edge receiver based on config
    match &config.edge {
        EdgeConfig::Mock {
            reading_interval_secs,
            status_interval_secs,
            device_count,
        } => {
            info!(
                reading_interval_secs,
                status_interval_secs, device_count, "Using mock edge receiver"
            );

            let receiver = MockEdgeReceiver::new(
                dispatcher_id,
                location,
                *reading_interval_secs,
                *status_interval_secs,
                *device_count,
            );
            run_edge_receiver(receiver, cancel, storage, dispatcher_id, location, config).await?;
        }
        EdgeConfig::Tcp { addr } => {
            info!(?addr, "Started TCP edge receiver");

            let receiver = TcpEdgeReceiver::new(*addr, dispatcher_id);
            run_edge_receiver(receiver, cancel, storage, dispatcher_id, location, config).await?;
        }
    };

    Ok(())
}

async fn run_edge_receiver<E: EdgeReceiver, S>(
    edge_receiver: E,
    cancel: CancellationToken,
    storage: S,
    dispatcher_id: DispatcherId,
    location: H3Cell,
    config: Config,
) -> color_eyre::Result<()>
where
    S: SensorReadingsStorage + DeviceStatusStorage + Clone + Send + Sync + 'static,
    <S as SensorReadingsStorage>::Error: std::error::Error + Send + Sync + 'static,
    <S as DeviceStatusStorage>::Error: std::error::Error + Send + Sync + 'static,
{
    // Start edge receiver
    let edge_rx = edge_receiver.start(cancel.clone()).await?;

    // Spawn data collector task
    let storage_for_collector = storage.clone();
    let cancel_for_collector = cancel.clone();
    let collector_handle = tokio::spawn(async move {
        run_data_collector(edge_rx, storage_for_collector, cancel_for_collector).await;
    });

    // Spawn uploader task
    let storage_for_uploader = storage.clone();
    let cancel_for_uploader = cancel.clone();
    let prime_addr = config.prime.rpc_addr;
    let upload_interval = Duration::from_secs(config.prime.upload_interval_secs);
    let uploader_handle = tokio::spawn(async move {
        run_uploader(
            storage_for_uploader,
            prime_addr,
            dispatcher_id,
            location,
            upload_interval,
            cancel_for_uploader,
        )
        .await;
    });

    // HTTP server
    let http_addr = config.server.http_addr;
    let axum_app = Router::new().route("/health", get(health_handler));
    let axum_listener = TcpListener::bind(http_addr).await?;
    info!(%http_addr, "HTTP server listening");

    let cancel_for_http = cancel.clone();

    tokio::select! {
        result = axum::serve(axum_listener, axum_app).with_graceful_shutdown(async move {
            cancel_for_http.cancelled().await;
        }) => {
            if let Err(e) = result {
                error!(error = ?e, "HTTP server error");
            }
            info!("HTTP server shut down");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
            cancel.cancel();
        }
    }

    // Wait for background tasks to complete
    let _ = collector_handle.await;
    let _ = uploader_handle.await;

    info!("ersha-dispatch shut down complete");
    Ok(())
}

async fn run_data_collector<S>(
    mut edge_rx: mpsc::Receiver<EdgeData>,
    storage: S,
    cancel: CancellationToken,
) where
    S: SensorReadingsStorage + DeviceStatusStorage,
    <S as SensorReadingsStorage>::Error: std::error::Error,
    <S as DeviceStatusStorage>::Error: std::error::Error,
{
    info!("Data collector started");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Data collector shutting down");
                break;
            }
            Some(data) = edge_rx.recv() => {
                match data {
                    EdgeData::Reading(reading) => {
                        let reading_id = reading.id;
                        if let Err(e) = SensorReadingsStorage::store(&storage, reading).await {
                            error!(error = ?e, reading_id = ?reading_id, "Failed to store reading");
                        } else {
                            info!(reading_id = ?reading_id, "Stored sensor reading");
                        }
                    }
                    EdgeData::Status(status) => {
                        let status_id = status.id;
                        if let Err(e) = DeviceStatusStorage::store(&storage, status).await {
                            error!(error = ?e, status_id = ?status_id, "Failed to store status");
                        } else {
                            info!(status_id = ?status_id, "Stored device status");
                        }
                    }
                }
            }
        }
    }
}

async fn run_uploader<S>(
    storage: S,
    prime_addr: std::net::SocketAddr,
    dispatcher_id: DispatcherId,
    location: H3Cell,
    upload_interval: Duration,
    cancel: CancellationToken,
) where
    S: SensorReadingsStorage + DeviceStatusStorage,
    <S as SensorReadingsStorage>::Error: std::error::Error,
    <S as DeviceStatusStorage>::Error: std::error::Error,
{
    info!(
        prime_addr = %prime_addr,
        upload_interval_secs = upload_interval.as_secs(),
        "Uploader started"
    );

    let mut interval = tokio::time::interval(upload_interval);
    let mut client: Option<Client> = None;
    let mut backoff = Duration::from_secs(1);
    const MAX_BACKOFF: Duration = Duration::from_secs(60);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Uploader shutting down");
                break;
            }
            _ = interval.tick() => {
                // Ensure we have a connected and registered client
                if client.is_none() {
                    match connect_and_register(prime_addr, dispatcher_id, location).await {
                        Ok(c) => {
                            client = Some(c);
                            backoff = Duration::from_secs(1);
                        }
                        Err(e) => {
                            warn!(error = %e, backoff_secs = backoff.as_secs(), "Failed to connect to ersha-prime, will retry");
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(MAX_BACKOFF);
                            continue;
                        }
                    }
                }

                let c = client.as_ref().unwrap();

                // Fetch pending data
                let readings = match SensorReadingsStorage::fetch_pending(&storage).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!(error = ?e, "Failed to fetch pending readings");
                        continue;
                    }
                };

                let statuses = match DeviceStatusStorage::fetch_pending(&storage).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(error = ?e, "Failed to fetch pending statuses");
                        continue;
                    }
                };

                if readings.is_empty() && statuses.is_empty() {
                    tracing::debug!("No pending data to upload");
                    continue;
                }

                info!(
                    readings_count = readings.len(),
                    statuses_count = statuses.len(),
                    "Uploading batch to ersha-prime"
                );

                // Collect IDs for marking as uploaded
                let reading_ids: Vec<_> = readings.iter().map(|r| r.id).collect();
                let status_ids: Vec<_> = statuses.iter().map(|s| s.id).collect();

                let batch = BatchUploadRequest {
                    id: BatchId(Ulid::new()),
                    dispatcher_id,
                    readings: readings.into_boxed_slice(),
                    statuses: statuses.into_boxed_slice(),
                    timestamp: jiff::Timestamp::now(),
                };

                match c.batch_upload(batch).await {
                    Ok(resp) => {
                        info!(batch_id = ?resp.id, "Batch uploaded successfully");

                        // Mark data as uploaded
                        if let Err(e) = SensorReadingsStorage::mark_uploaded(&storage, &reading_ids).await {
                            error!(error = ?e, "Failed to mark readings as uploaded");
                        }
                        if let Err(e) = DeviceStatusStorage::mark_uploaded(&storage, &status_ids).await {
                            error!(error = ?e, "Failed to mark statuses as uploaded");
                        }
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to upload batch, will reconnect");
                        client = None;
                    }
                }
            }
        }
    }
}

async fn connect_and_register(
    prime_addr: std::net::SocketAddr,
    dispatcher_id: DispatcherId,
    location: H3Cell,
) -> color_eyre::Result<Client> {
    let stream = TcpStream::connect(prime_addr).await?;
    let client = Client::new(stream);

    let hello = HelloRequest {
        dispatcher_id,
        location,
    };

    let resp = client.hello(hello).await?;
    info!(dispatcher_id = ?resp.dispatcher_id, "Registered with ersha-prime");

    Ok(client)
}

async fn health_handler() -> &'static str {
    "OK"
}
