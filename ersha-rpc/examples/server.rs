use ersha_core::{BatchUploadRequest, BatchUploadResponse, HelloRequest, HelloResponse};
use ersha_rpc::Server;
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("tracing=info,server=info,ersha_rpc=debug")
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let bind_addr = "127.0.0.1:8080".to_string();

    info!("starting server on {}", bind_addr);

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

    let server = Server::new(listener)
        .on_ping(|_msg_id, _rpc| async move {
            info!("received ping, responding with pong");
        })
        .on_hello(|hello: HelloRequest, _msg_id, _rpc| async move {
            info!("received hello request from dispatcher {:?} at location {:?}", 
                  hello.dispatcher_id, hello.location);

            HelloResponse {
                dispatcher_id: hello.dispatcher_id,
            }
        })
        .on_batch_upload(|request: BatchUploadRequest, _msg_id, _rpc| async move {
            info!("received batch upload request: batch_id = {:?}, dispatcher_id = {:?}, readings = {}, statuses = {}", 
                  request.id, request.dispatcher_id, request.readings.len(), request.statuses.len());
            BatchUploadResponse {
                id: request.id,
            }
        });

    info!("server ready, accepting connections...");
    server.serve().await;
}
