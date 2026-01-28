use std::sync::Arc;

use ersha_core::{DispatcherId, H3Cell, HelloRequest, HelloResponse};
use ersha_rpc::Client;
use ersha_tls::TlsConfig;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, rustls::pki_types::ServerName};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "tracing=info,client=info,ersha_rpc=debug".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let server_addr = "127.0.0.1:19080".to_string();

    info!("connecting to server at {}", server_addr);

    let rustls_config = ersha_tls::client_config(&TlsConfig {
        cert: "./examples/keys/client.crt".into(),
        key: "./examples/keys/client.key".into(),
        root_ca: "./examples/keys/root_ca.crt".into(),
        domain: "localhost".into(),
    })
    .expect("Unable to build client rustls config");
    let connector = TlsConnector::from(Arc::new(rustls_config));

    let server_name = ServerName::try_from("localhost").expect("Invalid `ServerName`");

    let stream = match TcpStream::connect(&server_addr).await {
        Ok(stream) => {
            info!("connected to server");
            connector
                .connect(server_name, stream)
                .await
                .expect("Tls Handshake faild")
        }
        Err(e) => {
            error!("failed to connect to server: {}", e);
            std::process::exit(1);
        }
    };

    let client = Client::new(stream);

    info!("sending ping...");
    match client.ping().await {
        Ok(()) => {
            info!("ping successful!");
        }
        Err(e) => {
            error!("ping failed: {}", e);
            std::process::exit(1);
        }
    }

    info!("sending hello request...");
    let hello_request = HelloRequest {
        dispatcher_id: DispatcherId(ulid::Ulid::new()),
        location: H3Cell(0x8a2a1072b59ffff), // Example H3 cell
    };

    match client.hello(hello_request).await {
        Ok(HelloResponse::Accepted { dispatcher_id }) => {
            info!(
                "hello response received: dispatcher_id = {:?}",
                dispatcher_id
            );
        }
        Ok(HelloResponse::Rejected { reason }) => {
            error!("hello request rejected: {:?}", reason);
            std::process::exit(1);
        }
        Err(e) => {
            error!("hello request failed: {}", e);
            std::process::exit(1);
        }
    }

    info!("Client operations completed successfully");
}
