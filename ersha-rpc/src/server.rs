use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

use crate::{MessageId, RpcTcp, WireMessage};
use ersha_core::{BatchUploadRequest, BatchUploadResponse, HelloRequest, HelloResponse};

pub type HandlerFn<Req, Res> =
    Box<dyn Fn(Req, MessageId, &RpcTcp) -> Pin<Box<dyn Future<Output = Res> + Send>> + Send + Sync>;

pub struct Server {
    listener: TcpListener,
    buffer_size: usize,
    handlers: Arc<ServerHandlers>,
}

struct ServerHandlers {
    on_ping: Option<HandlerFn<(), ()>>,
    on_hello: Option<HandlerFn<HelloRequest, HelloResponse>>,
    on_batch_upload: Option<HandlerFn<BatchUploadRequest, BatchUploadResponse>>,
}

impl Server {
    pub fn new(listener: TcpListener) -> Self {
        Self {
            listener,
            buffer_size: 1024,
            handlers: Arc::new(ServerHandlers {
                on_hello: None,
                on_ping: None,
                on_batch_upload: None,
            }),
        }
    }

    pub fn with_buffer(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn on_hello<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(HelloRequest, MessageId, &RpcTcp) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HelloResponse> + Send + 'static,
    {
        let handlers = Arc::get_mut(&mut self.handlers).unwrap();
        handlers.on_hello = Some(Box::new(move |hello, msg_id, rpc| {
            Box::pin(handler(hello, msg_id, rpc))
        }));
        self
    }

    pub fn on_ping<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(MessageId, &RpcTcp) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let handlers = Arc::get_mut(&mut self.handlers).unwrap();
        handlers.on_ping = Some(Box::new(move |_, msg_id, rpc| {
            Box::pin(handler(msg_id, rpc))
        }));
        self
    }

    pub fn on_batch_upload<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(BatchUploadRequest, MessageId, &RpcTcp) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = BatchUploadResponse> + Send + 'static,
    {
        let handlers = Arc::get_mut(&mut self.handlers).unwrap();
        handlers.on_batch_upload = Some(Box::new(move |request, msg_id, rpc| {
            Box::pin(handler(request, msg_id, rpc))
        }));
        self
    }

    async fn handle_connection(
        handlers: Arc<ServerHandlers>,
        stream: TcpStream,
        buffer_size: usize,
    ) {
        let mut rpc = RpcTcp::new(stream, buffer_size);

        loop {
            let envelope = match rpc.recv().await {
                Some(env) => env,
                None => {
                    tracing::debug!("connection closed");
                    break;
                }
            };

            let msg_id = envelope.msg_id;
            let payload = envelope.payload;

            match payload {
                WireMessage::Ping => {
                    if let Some(handler) = &handlers.on_ping {
                        handler((), msg_id, &rpc).await;
                        let _ = rpc.reply(msg_id, WireMessage::Pong).await;
                    } else {
                        let _ = rpc.reply(msg_id, WireMessage::Pong).await;
                    }
                }
                WireMessage::HelloRequest(hello) => {
                    if let Some(handler) = &handlers.on_hello {
                        let response = handler(hello, msg_id, &rpc).await;
                        let _ = rpc
                            .reply(msg_id, WireMessage::HelloResponse(response))
                            .await;
                    } else {
                        tracing::warn!("received Hello but no handler registered");
                    }
                }
                WireMessage::BatchUploadRequest(request) => {
                    if let Some(handler) = &handlers.on_batch_upload {
                        let response = handler(request, msg_id, &rpc).await;
                        let _ = rpc
                            .reply(msg_id, WireMessage::BatchUploadResponse(response))
                            .await;
                    } else {
                        tracing::warn!("received BatchUploadRequest but no handler registered");
                    }
                }
                WireMessage::Pong => {
                    tracing::debug!("received Pong (unexpected on server)");
                }
                WireMessage::HelloResponse(res) => {
                    tracing::debug!("received HelloResponse (unexpected on server): {res:?}");
                }
                WireMessage::BatchUploadResponse(res) => {
                    tracing::debug!("received BatchUploadResponse (unexpected on server): {res:?}");
                }
                WireMessage::Error(err) => {
                    tracing::warn!("received error: {:?}", err);
                }
            }
        }
    }

    pub async fn serve(&self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    let handlers = self.handlers.clone();
                    let buffer_size = self.buffer_size;
                    tokio::spawn(async move {
                        Self::handle_connection(handlers, stream, buffer_size).await;
                    });
                }
                Err(e) => {
                    tracing::error!("error accepting connection: {:?}", e);
                }
            }
        }
    }
}
