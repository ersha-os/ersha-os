use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::{MessageId, RpcTcp, WireMessage};
use ersha_core::{BatchUploadRequest, BatchUploadResponse, HelloRequest, HelloResponse};

pub type HandlerFn<Req, Res, S> = Box<
    dyn Fn(Req, MessageId, &RpcTcp, &S) -> Pin<Box<dyn Future<Output = Res> + Send>> + Send + Sync,
>;

pub struct Server<S> {
    listener: TcpListener,
    buffer_size: usize,
    state: Arc<S>,
    handlers: ServerHandlers<S>,
}

struct ServerHandlers<S> {
    on_ping: Option<HandlerFn<(), (), S>>,
    on_hello: Option<HandlerFn<HelloRequest, HelloResponse, S>>,
    on_batch_upload: Option<HandlerFn<BatchUploadRequest, BatchUploadResponse, S>>,
}

impl<S: Send + Sync + 'static> Server<S> {
    pub fn new(listener: TcpListener, state: S) -> Self {
        Self {
            listener,
            buffer_size: 1024,
            state: Arc::new(state),
            handlers: ServerHandlers {
                on_hello: None,
                on_ping: None,
                on_batch_upload: None,
            },
        }
    }

    pub fn with_buffer(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn on_hello<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(HelloRequest, MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HelloResponse> + Send + 'static,
    {
        self.handlers.on_hello = Some(Box::new(move |hello, msg_id, rpc, state| {
            Box::pin(handler(hello, msg_id, rpc, state))
        }));
        self
    }

    pub fn on_ping<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.handlers.on_ping = Some(Box::new(move |_, msg_id, rpc, state| {
            Box::pin(handler(msg_id, rpc, state))
        }));
        self
    }

    pub fn on_batch_upload<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(BatchUploadRequest, MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = BatchUploadResponse> + Send + 'static,
    {
        self.handlers.on_batch_upload = Some(Box::new(move |request, msg_id, rpc, state| {
            Box::pin(handler(request, msg_id, rpc, state))
        }));
        self
    }

    async fn handle_connection(
        handlers: Arc<ServerHandlers<S>>,
        state: Arc<S>,
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
                        handler((), msg_id, &rpc, &state).await;
                    }
                    if let Err(e) = rpc.reply(msg_id, WireMessage::Pong).await {
                        tracing::error!("failed to send Pong reply: {:?}", e);
                    }
                }
                WireMessage::HelloRequest(hello) => {
                    if let Some(handler) = &handlers.on_hello {
                        let response = handler(hello, msg_id, &rpc, &state).await;
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::HelloResponse(response))
                            .await
                        {
                            tracing::error!("failed to send HelloResponse reply: {:?}", e);
                        }
                    } else {
                        tracing::warn!("received HelloRequest but no handler registered");
                    }
                }
                WireMessage::BatchUploadRequest(request) => {
                    if let Some(handler) = &handlers.on_batch_upload {
                        let response = handler(request, msg_id, &rpc, &state).await;
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::BatchUploadResponse(response))
                            .await
                        {
                            tracing::error!("failed to send BatchUploadResponse reply: {:?}", e);
                        }
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

    pub async fn serve(self, cancel: CancellationToken) {
        let handlers = Arc::new(self.handlers);
        let state = self.state;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("server shutdown requested");
                    break;
                }
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            tracing::debug!("accepted connection from {:?}", addr);
                            let handlers = handlers.clone();
                            let state = state.clone();
                            let buffer_size = self.buffer_size;
                            tokio::spawn(async move {
                                Self::handle_connection(handlers, state, stream, buffer_size).await;
                            });
                        }
                        Err(e) => {
                            tracing::error!("error accepting connection: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}
