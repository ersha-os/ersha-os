# ersha-rpc

An async, TLS-based RPC crate for communication between [dispatchers](../ersha-dispatch) and the [central server](../ersha-prime) in the Ersha system.

Built on top of tokio with postcard binary serialization and length-prefixed framing.

## Overview

ersha-rpc provides two main abstractions:

- **`Client`** — connects to a server and makes typed request/response calls
- **`Server`** — accepts TLS connections and dispatches incoming messages to registered handlers

Messages are serialized with [postcard](https://docs.rs/postcard), framed with a 4-byte length prefix (max 2 MB per frame), and correlated via ULID-based message IDs.

## Usage

### Client

```rust
use ersha_rpc::Client;
use ersha_core::{HelloRequest, HelloResponse, DispatcherId, H3Cell};

// `stream` is any AsyncRead + AsyncWrite (typically a TLS stream)
let client = Client::new(stream);

// Health check
client.ping().await?;

// Register with the server
let response = client.hello(HelloRequest {
    dispatcher_id: DispatcherId(ulid::Ulid::new()),
    location: H3Cell(0x8a2a1072b59ffff),
}).await?;

match response {
    HelloResponse::Accepted { dispatcher_id } => { /* registered */ }
    HelloResponse::Rejected { reason } => { /* handle rejection */ }
}

// Send data
client.batch_upload(batch_request).await?;
client.alert(alert_request).await?;
client.dispatcher_status(status_request).await?;
client.device_disconnection(disconnection_request).await?;
```

The client has a default 5-second timeout per call, configurable via `Client::with_timeout`.

### Server

```rust
use ersha_rpc::{Server, CancellationToken};
use ersha_core::{HelloRequest, HelloResponse, BatchUploadRequest, BatchUploadResponse};

struct AppState { /* ... */ }

let server = Server::new(tcp_listener, AppState { /* ... */ }, tls_acceptor)
    .on_ping(|_msg_id, _rpc, _state| async {})
    .on_hello(|req: HelloRequest, _msg_id, _rpc, _state| async move {
        HelloResponse::Accepted {
            dispatcher_id: req.dispatcher_id,
        }
    })
    .on_batch_upload(|req: BatchUploadRequest, _msg_id, _rpc, _state| async move {
        BatchUploadResponse {
            id: req.id,
            readings_stored: req.readings.len() as u32,
            readings_rejected: 0,
            statuses_stored: req.statuses.len() as u32,
            statuses_rejected: 0,
        }
    });

let cancel = CancellationToken::new();
server.serve(cancel).await;
```

Handlers receive the request payload, the message ID, a reference to the underlying `RpcTcp` connection, and shared application state. The server spawns a task per connection and runs until the `CancellationToken` is cancelled.

## Message Types

All RPC messages are defined in the `WireMessage` enum:

| Request | Response | Description |
|---------|----------|-------------|
| `Ping` | `Pong` | Health check |
| `HelloRequest` | `HelloResponse` | Dispatcher registration |
| `BatchUploadRequest` | `BatchUploadResponse` | Sensor data upload |
| `AlertRequest` | `AlertResponse` | Alert notification |
| `DispatcherStatusRequest` | `DispatcherStatusResponse` | Status heartbeat |
| `DeviceDisconnectionRequest` | `DeviceDisconnectionResponse` | Device lifecycle event |

Any message can also be answered with `Error(WireError)`.

## Low-level API

For more control, use `RpcTcp` directly:

```rust
use ersha_rpc::{RpcTcp, WireMessage};
use std::time::Duration;

let mut rpc = RpcTcp::new(stream, 1024);

// Fire-and-forget
rpc.send(WireMessage::Ping).await?;

// Request-response with timeout
let response = rpc.call(WireMessage::Ping, Duration::from_secs(5)).await?;

// Reply to an incoming message
let envelope = rpc.recv().await.unwrap();
rpc.reply(envelope.msg_id, WireMessage::Pong).await?;
```

## Examples

Working client and server examples with TLS are in the [`examples/`](./examples) directory:

```sh
# Run the server
cargo run --example server

# In another terminal, run the client
cargo run --example client
```
