use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ersha_core::{BatchUploadRequest, BatchUploadResponse, HelloRequest, HelloResponse};
use ersha_rpc::{CancellationToken, Server};
use ersha_tls::TlsConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "tracing=info,server=info,ersha_rpc=debug".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let bind_addr = "127.0.0.1:19080".to_string();

    info!("starting server on {}", bind_addr);

    let rusttls_config = ersha_tls::server_config(&TlsConfig {
        cert: "./examples/keys/server.crt".into(),
        key: "./examples/keys/server.key".into(),
        root_ca: "./examples/keys/root_ca.crt".into(),
        domain: "localhost".into(),
    })
    .expect("Unable to build rustls server config");
    let acceptor = TlsAcceptor::from(Arc::new(rusttls_config));

    let listener = match TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            info!("server listening on {}", bind_addr);
            listener
        }
        Err(e) => {
            error!("failed to bind to {}: {}", bind_addr, e);
            std::process::exit(1);
        }
    };

    // Define application state
    #[derive(Clone)]
    struct AppState {
        request_count: Arc<AtomicUsize>,
    }

    let state = AppState {
        request_count: Arc::new(AtomicUsize::new(0)),
    };

    let server = Server::new(listener, state, acceptor)
        .on_ping(|_msg_id, _rpc, state| {
            let counter = state.request_count.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                info!("received ping #{}, responding with pong", count);
            }
        })
        .on_hello(|hello: HelloRequest, _msg_id, _rpc, state| {
            let counter = state.request_count.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                info!(
                    "received hello request #{} from dispatcher {:?} at location {:?}",
                    count, hello.dispatcher_id, hello.location
                );

                HelloResponse::Accepted {
                    dispatcher_id: hello.dispatcher_id,
                }
            }
        })
        .on_batch_upload(|request: BatchUploadRequest, _msg_id, _rpc, state| {
            let counter = state.request_count.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let readings_count = request.readings.len() as u32;
                let statuses_count = request.statuses.len() as u32;
                info!(
                    "received batch upload request #{}: batch_id = {:?}, dispatcher_id = {:?}, readings = {}, statuses = {}",
                    count,
                    request.id,
                    request.dispatcher_id,
                    readings_count,
                    statuses_count
                );
                BatchUploadResponse {
                    id: request.id,
                    readings_stored: readings_count,
                    readings_rejected: 0,
                    statuses_stored: statuses_count,
                    statuses_rejected: 0,
                }
            }
        });

    // Set up graceful shutdown on Ctrl+C
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c");
        info!("received ctrl-c, shutting down...");
        cancel_clone.cancel();
    });

    info!("server ready, accepting connections... (press Ctrl+C to stop)");
    server.serve(cancel).await;
    info!("server stopped");
}
