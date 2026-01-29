use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{Router, routing::get};
use clap::Parser;
use ersha_core::{
    AlertId, AlertRequest, AlertSeverity, AlertType, BatchId, BatchUploadRequest,
    DeviceDisconnectionRequest, DispatcherId, DispatcherStatusRequest, H3Cell, HelloRequest,
    HelloResponse, SensorState,
};
use ersha_dispatch::edge::tcp::TcpEdgeReceiver;
use ersha_dispatch::{
    Config, DeviceStatusStorage, DispatcherState, EdgeConfig, EdgeData, EdgeReceiver,
    MemoryStorage, MockDeviceInfo, MockEdgeReceiver, PrimeEvent, SensorReadingsStorage,
    SqliteStorage, StorageConfig,
};
use ersha_rpc::Client;
use ersha_tls::TlsConfig;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;
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

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

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
    let state = DispatcherState::new();

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
                *reading_interval_secs,
                *status_interval_secs,
                *device_count,
            );

            // Register dispatcher and mock devices with ersha-prime via HTTP API
            let prime_http_url =
                format!("http://{}", config.prime.rpc_addr).replace(":9000", ":8080");
            register_mock_entities(
                &prime_http_url,
                dispatcher_id,
                location,
                &receiver.device_info(),
            )
            .await;

            run_edge_receiver(
                receiver,
                cancel,
                storage,
                dispatcher_id,
                location,
                config,
                state,
            )
            .await?;
        }
        EdgeConfig::Tcp { addr } => {
            info!(?addr, "Started TCP edge receiver");

            let receiver = TcpEdgeReceiver::new(*addr, dispatcher_id, state.clone());
            run_edge_receiver(
                receiver,
                cancel,
                storage,
                dispatcher_id,
                location,
                config,
                state,
            )
            .await?;
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
    state: DispatcherState,
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
    let state_for_collector = state.clone();
    let collector_handle = tokio::spawn(async move {
        run_data_collector(
            edge_rx,
            storage_for_collector,
            cancel_for_collector,
            state_for_collector,
            dispatcher_id,
        )
        .await;
    });

    // Spawn uploader task
    let storage_for_uploader = storage.clone();
    let cancel_for_uploader = cancel.clone();
    let state_for_uploader = state.clone();
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
            state_for_uploader,
            config.tls,
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
    state: DispatcherState,
    dispatcher_id: DispatcherId,
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
                        let device_id = status.device_id;

                        // Check for critical battery (< 10%)
                        if status.battery_percent.0 < 10 {
                            let alert = AlertRequest {
                                id: AlertId(Ulid::new()),
                                dispatcher_id,
                                device_id: Some(device_id),
                                severity: AlertSeverity::Critical,
                                alert_type: AlertType::CriticalBattery,
                                message: format!(
                                    "Device battery critically low: {}%",
                                    status.battery_percent.0
                                )
                                .into(),
                                timestamp: jiff::Timestamp::now(),
                            };
                            state.queue_alert(alert).await;
                            warn!(
                                device_id = ?device_id,
                                battery_percent = status.battery_percent.0,
                                "Critical battery alert queued"
                            );
                        }

                        // Check for sensor failures
                        for sensor_status in status.sensor_statuses.iter() {
                            if sensor_status.state == SensorState::Faulty {
                                let alert = AlertRequest {
                                    id: AlertId(Ulid::new()),
                                    dispatcher_id,
                                    device_id: Some(device_id),
                                    severity: AlertSeverity::Warning,
                                    alert_type: AlertType::SensorFailure,
                                    message: format!(
                                        "Sensor {:?} reported faulty state",
                                        sensor_status.sensor_id
                                    )
                                    .into(),
                                    timestamp: jiff::Timestamp::now(),
                                };
                                state.queue_alert(alert).await;
                                warn!(
                                    device_id = ?device_id,
                                    sensor_id = ?sensor_status.sensor_id,
                                    "Sensor failure alert queued"
                                );
                            }
                        }

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

#[allow(clippy::too_many_arguments)]
async fn run_uploader<S>(
    storage: S,
    prime_addr: std::net::SocketAddr,
    dispatcher_id: DispatcherId,
    location: H3Cell,
    upload_interval: Duration,
    cancel: CancellationToken,
    state: DispatcherState,
    tls_config: TlsConfig,
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
                    match connect_and_register(prime_addr, dispatcher_id, location, &tls_config).await {
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

                // Send dispatcher status
                let pending_events = state.take_pending_events().await;
                let status_request = DispatcherStatusRequest {
                    dispatcher_id,
                    connected_devices: state.connected_count().await,
                    uptime_seconds: state.uptime_secs().await,
                    pending_uploads: pending_events.len() as u32,
                    timestamp: jiff::Timestamp::now(),
                };

                match c.dispatcher_status(status_request).await {
                    Ok(_) => {
                        tracing::debug!("Dispatcher status sent to ersha-prime");
                    }
                    Err(e) => {
                        error!(error = ?e, "Failed to send dispatcher status, will reconnect");
                        client = None;
                        continue;
                    }
                }

                // Process pending events (disconnections and alerts)
                for event in pending_events {
                    match event {
                        PrimeEvent::DeviceDisconnection { device_id, reason } => {
                            let request = DeviceDisconnectionRequest {
                                device_id,
                                dispatcher_id,
                                timestamp: jiff::Timestamp::now(),
                                reason: Some(reason),
                            };
                            match c.device_disconnection(request).await {
                                Ok(_) => {
                                    info!(device_id = ?device_id, "Device disconnection sent to ersha-prime");
                                }
                                Err(e) => {
                                    error!(error = ?e, device_id = ?device_id, "Failed to send device disconnection");
                                }
                            }
                        }
                        PrimeEvent::Alert(alert) => {
                            let alert_id = alert.id;
                            match c.alert(alert).await {
                                Ok(_) => {
                                    info!(alert_id = ?alert_id, "Alert sent to ersha-prime");
                                }
                                Err(e) => {
                                    error!(error = ?e, alert_id = ?alert_id, "Failed to send alert");
                                }
                            }
                        }
                    }
                }

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
    tls_config: &TlsConfig,
) -> color_eyre::Result<Client> {
    let stream = TcpStream::connect(prime_addr).await?;

    let rustls_config = ersha_tls::client_config(tls_config)?;
    let connector = TlsConnector::from(Arc::new(rustls_config));

    let server_name = ServerName::try_from(tls_config.domain.clone())?;

    let tls_stream = connector.connect(server_name, stream).await?;

    let client = Client::new(tls_stream);

    let hello = HelloRequest {
        dispatcher_id,
        location,
    };

    let resp = client.hello(hello).await?;
    match resp {
        HelloResponse::Accepted { dispatcher_id } => {
            info!(dispatcher_id = ?dispatcher_id, "Registered with ersha-prime");
            Ok(client)
        }
        HelloResponse::Rejected { reason } => Err(color_eyre::eyre::eyre!(
            "Connection rejected by ersha-prime: {:?}",
            reason
        )),
    }
}

async fn health_handler() -> &'static str {
    "OK"
}

/// Sensor kinds in the same order as MockDevice creates them.
const SENSOR_KINDS: [&str; 5] = [
    "soil_moisture",
    "soil_temp",
    "air_temp",
    "humidity",
    "rainfall",
];

/// Register the dispatcher and all mock devices with ersha-prime's HTTP API.
///
/// This is best-effort: if prime is unreachable or already has the entities
/// registered, we log and continue.
async fn register_mock_entities(
    prime_http_url: &str,
    dispatcher_id: DispatcherId,
    location: H3Cell,
    devices: &[MockDeviceInfo],
) {
    let http = reqwest::Client::new();

    // Register dispatcher
    let dispatcher_body = serde_json::json!({
        "id": dispatcher_id.0.to_string(),
        "location": location.0,
    });
    match http
        .post(format!("{prime_http_url}/api/dispatchers"))
        .json(&dispatcher_body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!("Registered dispatcher with ersha-prime");
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(%status, body, "Failed to register dispatcher (may already exist)");
        }
        Err(e) => {
            warn!(error = %e, "Could not reach ersha-prime to register dispatcher");
        }
    }

    // Register devices
    let mut registered = 0u32;
    let mut failed = 0u32;
    for device in devices {
        let sensors: Vec<serde_json::Value> = device
            .sensor_ids
            .iter()
            .zip(SENSOR_KINDS.iter())
            .map(|(sid, kind)| {
                serde_json::json!({
                    "id": sid.0.to_string(),
                    "kind": kind,
                })
            })
            .collect();

        let body = serde_json::json!({
            "id": device.device_id.0.to_string(),
            "location": device.location.0,
            "kind": "sensor",
            "manufacturer": "ersha-mock",
            "sensors": sensors,
        });

        match http
            .post(format!("{prime_http_url}/api/devices"))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                registered += 1;
            }
            Ok(_) | Err(_) => {
                failed += 1;
            }
        }
    }

    info!(
        registered,
        failed,
        total = devices.len(),
        "Mock device registration complete"
    );
}
