use std::path::PathBuf;
use std::{net::SocketAddr, sync::Arc};

use axum::routing::get;
use clap::Parser;
use ersha_core::{
    AlertRequest, AlertResponse, BatchUploadRequest, BatchUploadResponse,
    DeviceDisconnectionRequest, DeviceDisconnectionResponse, DispatcherState,
    DispatcherStatusRequest, DispatcherStatusResponse, HelloRejectionReason, HelloRequest,
    HelloResponse,
};
use ersha_prime::{
    api,
    config::{Config, RegistryConfig},
    registry::{
        DeviceRegistry, DeviceStatusRegistry, DispatcherRegistry, ReadingRegistry,
        clickhouse::{
            ClickHouseDeviceRegistry, ClickHouseDeviceStatusRegistry, ClickHouseDispatcherRegistry,
            ClickHouseReadingRegistry,
        },
        memory::{
            InMemoryDeviceRegistry, InMemoryDeviceStatusRegistry, InMemoryDispatcherRegistry,
            InMemoryReadingRegistry,
        },
        sqlite::{
            SqliteDeviceRegistry, SqliteDeviceStatusRegistry, SqliteDispatcherRegistry,
            SqliteReadingRegistry,
        },
    },
};
use ersha_rpc::Server;
use ersha_tls::TlsConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "ersha-prime")]
#[command(about = "Ersha Prime")]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, default_value = "ersha-prime.toml")]
    config: PathBuf,
}

struct AppState<D, Dev, R, S>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
    R: ReadingRegistry,
    S: DeviceStatusRegistry,
{
    dispatcher_registry: D,
    device_registry: Dev,
    reading_registry: R,
    device_status_registry: S,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "tracing=info,ersha_prime=info".to_owned());
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

    info!(rpc_addr = %config.server.rpc_addr, http_addr = %config.server.http_addr, "Starting servers");

    match config.registry {
        RegistryConfig::Memory => {
            info!("Using in-memory registries");
            let dispatcher_registry = InMemoryDispatcherRegistry::new();
            let device_registry = InMemoryDeviceRegistry::new();
            let reading_registry = InMemoryReadingRegistry::new();
            let device_status_registry = InMemoryDeviceStatusRegistry::new();
            run_server(
                dispatcher_registry,
                device_registry,
                reading_registry,
                device_status_registry,
                config.server.rpc_addr,
                config.server.http_addr,
                config.tls,
            )
            .await?;
        }
        RegistryConfig::Sqlite { path } => {
            info!(path = ?path, "Using SQLite registries");
            let path_str = path.to_string_lossy();
            let dispatcher_registry = SqliteDispatcherRegistry::new(&path_str).await?;
            let device_registry = SqliteDeviceRegistry::new(&path_str).await?;
            let reading_registry = SqliteReadingRegistry::new(&path_str).await?;
            let device_status_registry = SqliteDeviceStatusRegistry::new(&path_str).await?;
            run_server(
                dispatcher_registry,
                device_registry,
                reading_registry,
                device_status_registry,
                config.server.rpc_addr,
                config.server.http_addr,
                config.tls,
            )
            .await?;
        }
        RegistryConfig::Clickhouse { url, database } => {
            info!(url = %url, database = %database, "Using ClickHouse registries");
            let dispatcher_registry = ClickHouseDispatcherRegistry::new(&url, &database).await?;
            let device_registry = ClickHouseDeviceRegistry::new(&url, &database).await?;
            let reading_registry = ClickHouseReadingRegistry::new(&url, &database).await?;
            let device_status_registry =
                ClickHouseDeviceStatusRegistry::new(&url, &database).await?;
            run_server(
                dispatcher_registry,
                device_registry,
                reading_registry,
                device_status_registry,
                config.server.rpc_addr,
                config.server.http_addr,
                config.tls,
            )
            .await?;
        }
    }

    Ok(())
}

async fn run_server<D, Dev, R, S>(
    dispatcher_registry: D,
    device_registry: Dev,
    reading_registry: R,
    device_status_registry: S,
    rpc_addr: SocketAddr,
    http_addr: SocketAddr,
    tls_config: TlsConfig,
) -> color_eyre::Result<()>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
    R: ReadingRegistry,
    S: DeviceStatusRegistry,
{
    // Clone registries for HTTP API before moving them into AppState
    let api_dispatcher_registry = dispatcher_registry.clone();
    let api_device_registry = device_registry.clone();

    let state = AppState {
        dispatcher_registry,
        device_registry,
        reading_registry,
        device_status_registry,
    };

    let cancel = CancellationToken::new();

    let rpc_listener = TcpListener::bind(rpc_addr).await?;
    info!(%rpc_addr, "RPC server listening");

    let rustls_config = ersha_tls::server_config(&tls_config)?;
    let rpc_acceptor = TlsAcceptor::from(Arc::new(rustls_config));

    let rpc_server = Server::new(rpc_listener, state, rpc_acceptor)
        .on_hello(
            |hello: HelloRequest, _msg_id, _rpc, state: &AppState<D, Dev, R, S>| {
                let dispatcher_registry = state.dispatcher_registry.clone();
                async move {
                    info!(
                        dispatcher_id = ?hello.dispatcher_id,
                        location = ?hello.location,
                        "received hello request"
                    );

                    match dispatcher_registry.get(hello.dispatcher_id).await {
                        Ok(Some(dispatcher)) if dispatcher.state == DispatcherState::Active => {
                            info!(dispatcher_id = ?hello.dispatcher_id, "dispatcher validated");
                            HelloResponse::Accepted {
                                dispatcher_id: hello.dispatcher_id,
                            }
                        }
                        Ok(Some(_)) => {
                            warn!(dispatcher_id = ?hello.dispatcher_id, "dispatcher is suspended");
                            HelloResponse::Rejected {
                                reason: HelloRejectionReason::DispatcherSuspended,
                            }
                        }
                        Ok(None) => {
                            warn!(dispatcher_id = ?hello.dispatcher_id, "unknown dispatcher");
                            HelloResponse::Rejected {
                                reason: HelloRejectionReason::UnknownDispatcher,
                            }
                        }
                        Err(e) => {
                            error!(error = ?e, "failed to check dispatcher");
                            HelloResponse::Rejected {
                                reason: HelloRejectionReason::InternalError,
                            }
                        }
                    }
                }
            },
        )
        .on_batch_upload(
            |request: BatchUploadRequest, _msg_id, _rpc, state: &AppState<D, Dev, R, S>| {
                let device_registry = state.device_registry.clone();
                let reading_registry = state.reading_registry.clone();
                let device_status_registry = state.device_status_registry.clone();
                async move {
                    info!(
                        batch_id = ?request.id,
                        readings = request.readings.len(),
                        statuses = request.statuses.len(),
                        "batch upload received"
                    );

                    // Filter readings to only include known devices
                    let mut valid_readings = Vec::new();
                    let mut rejected_readings = 0u32;
                    for reading in request.readings.into_vec() {
                        match device_registry.get(reading.device_id).await {
                            Ok(Some(_)) => {
                                valid_readings.push(reading);
                            }
                            _ => {
                                rejected_readings += 1;
                                warn!(device_id = ?reading.device_id, "rejected reading from unknown device");
                            }
                        }
                    }

                    // Filter statuses to only include known devices
                    let mut valid_statuses = Vec::new();
                    let mut rejected_statuses = 0u32;
                    for status in request.statuses.into_vec() {
                        match device_registry.get(status.device_id).await {
                            Ok(Some(_)) => {
                                valid_statuses.push(status);
                            }
                            _ => {
                                rejected_statuses += 1;
                                warn!(device_id = ?status.device_id, "rejected status from unknown device");
                            }
                        }
                    }

                    let readings_stored = valid_readings.len() as u32;
                    let statuses_stored = valid_statuses.len() as u32;

                    // Store valid readings
                    if !valid_readings.is_empty()
                        && let Err(e) = reading_registry.batch_store(valid_readings).await
                    {
                        error!(error = ?e, "failed to store readings");
                    }

                    // Store valid statuses
                    if !valid_statuses.is_empty()
                        && let Err(e) = device_status_registry.batch_store(valid_statuses).await
                    {
                        error!(error = ?e, "failed to store statuses");
                    }

                    info!(
                        batch_id = ?request.id,
                        readings_stored,
                        rejected_readings,
                        statuses_stored,
                        rejected_statuses,
                        "batch upload processed"
                    );

                    BatchUploadResponse {
                        id: request.id,
                        readings_stored,
                        readings_rejected: rejected_readings,
                        statuses_stored,
                        statuses_rejected: rejected_statuses,
                    }
                }
            },
        )
        .on_alert(
            |request: AlertRequest, _msg_id, _rpc, _state: &AppState<D, Dev, R, S>| async move {

                info!(
                    alert_id = ?request.id,
                    dispatcher_id = ?request.dispatcher_id,
                    device_id = ?request.device_id,
                    severity = ?request.severity,
                    alert_type = ?request.alert_type,
                    message = %request.message,
                    "alert received"
                );

                // For now, just acknowledge the alert
                // In the future, this could trigger notifications, store alerts, etc.
                AlertResponse {
                    alert_id: request.id,
                    acknowledged: true,
                }
            },
        )
        .on_dispatcher_status(
            |request: DispatcherStatusRequest, _msg_id, _rpc, _state: &AppState<D, Dev, R, S>| async move {
                info!(
                    dispatcher_id = ?request.dispatcher_id,
                    connected_devices = request.connected_devices,
                    uptime_seconds = request.uptime_seconds,
                    pending_uploads = request.pending_uploads,
                    "dispatcher status received"
                );

                DispatcherStatusResponse {
                    dispatcher_id: request.dispatcher_id,
                }
            },
        )
        .on_device_disconnection(
            |request: DeviceDisconnectionRequest, _msg_id, _rpc, _state: &AppState<D, Dev, R, S>| async move {
                info!(
                    device_id = ?request.device_id,
                    dispatcher_id = ?request.dispatcher_id,
                    reason = ?request.reason,
                    "device disconnection notification received"
                );

                DeviceDisconnectionResponse {
                    device_id: request.device_id,
                }
            },
        );

    // Create the API router with dispatcher and device routes
    let api_router = api::api_router(api_dispatcher_registry, api_device_registry);

    // Merge with health endpoint
    let axum_app = api_router.route("/health", get(health_handler));

    let axum_listener = TcpListener::bind(http_addr).await?;
    info!(%http_addr, "HTTP server listening");

    let cancel_clone = cancel.clone();
    tokio::select! {
        _ = rpc_server.serve(cancel.clone()) => {
            info!("RPC server shut down");
        }
        result = axum::serve(axum_listener, axum_app).with_graceful_shutdown(async move {
            cancel_clone.cancelled().await;
        }) => {
            if let Err(e) = result {
                tracing::error!(error = ?e, "HTTP server error");
            }
            info!("HTTP server shut down");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
            cancel.cancel();
        }
    }

    Ok(())
}

async fn health_handler() -> &'static str {
    "OK"
}
