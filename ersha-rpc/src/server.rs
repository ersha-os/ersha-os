use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use tokio_util::sync::CancellationToken;

use crate::{MessageId, RpcTcp, WireMessage};
use ersha_core::{
    AlertRequest, AlertResponse, BatchUploadRequest, BatchUploadResponse,
    DeviceDisconnectionRequest, DeviceDisconnectionResponse, DispatcherStatusRequest,
    DispatcherStatusResponse, HelloRequest, HelloResponse,
};

pub type HandlerFn<Req, Res, S> = Box<
    dyn Fn(Req, MessageId, &RpcTcp, &S) -> Pin<Box<dyn Future<Output = Res> + Send>> + Send + Sync,
>;

pub struct Server<S> {
    listener: TcpListener,
    acceptor: TlsAcceptor,
    buffer_size: usize,
    state: Arc<S>,
    handlers: ServerHandlers<S>,
}

struct ServerHandlers<S> {
    on_ping: Option<HandlerFn<(), (), S>>,
    on_hello: Option<HandlerFn<HelloRequest, HelloResponse, S>>,
    on_batch_upload: Option<HandlerFn<BatchUploadRequest, BatchUploadResponse, S>>,
    on_alert: Option<HandlerFn<AlertRequest, AlertResponse, S>>,
    on_dispatcher_status: Option<HandlerFn<DispatcherStatusRequest, DispatcherStatusResponse, S>>,
    on_device_disconnection:
        Option<HandlerFn<DeviceDisconnectionRequest, DeviceDisconnectionResponse, S>>,
}

impl<S: Send + Sync + 'static> Server<S> {
    pub fn new(listener: TcpListener, state: S, acceptor: TlsAcceptor) -> Self {
        Self {
            listener,
            acceptor,
            buffer_size: 1024,
            state: Arc::new(state),
            handlers: ServerHandlers {
                on_hello: None,
                on_ping: None,
                on_batch_upload: None,
                on_alert: None,
                on_dispatcher_status: None,
                on_device_disconnection: None,
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

    pub fn on_alert<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(AlertRequest, MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AlertResponse> + Send + 'static,
    {
        self.handlers.on_alert = Some(Box::new(move |request, msg_id, rpc, state| {
            Box::pin(handler(request, msg_id, rpc, state))
        }));
        self
    }

    pub fn on_dispatcher_status<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(DispatcherStatusRequest, MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DispatcherStatusResponse> + Send + 'static,
    {
        self.handlers.on_dispatcher_status = Some(Box::new(move |request, msg_id, rpc, state| {
            Box::pin(handler(request, msg_id, rpc, state))
        }));
        self
    }

    pub fn on_device_disconnection<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(DeviceDisconnectionRequest, MessageId, &RpcTcp, &S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DeviceDisconnectionResponse> + Send + 'static,
    {
        self.handlers.on_device_disconnection =
            Some(Box::new(move |request, msg_id, rpc, state| {
                Box::pin(handler(request, msg_id, rpc, state))
            }));
        self
    }

    async fn handle_connection(
        handlers: Arc<ServerHandlers<S>>,
        state: Arc<S>,
        stream: TlsStream<TcpStream>,
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
                        let should_close = matches!(response, HelloResponse::Rejected { .. });
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::HelloResponse(response))
                            .await
                        {
                            tracing::error!("failed to send HelloResponse reply: {:?}", e);
                        }
                        if should_close {
                            tracing::info!("closing connection - dispatcher rejected");
                            break;
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
                WireMessage::AlertRequest(request) => {
                    if let Some(handler) = &handlers.on_alert {
                        let response = handler(request, msg_id, &rpc, &state).await;
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::AlertResponse(response))
                            .await
                        {
                            tracing::error!("failed to send AlertResponse reply: {:?}", e);
                        }
                    } else {
                        tracing::warn!("received AlertRequest but no handler registered");
                    }
                }
                WireMessage::AlertResponse(res) => {
                    tracing::debug!("received AlertResponse (unexpected on server): {res:?}");
                }
                WireMessage::DispatcherStatusRequest(request) => {
                    if let Some(handler) = &handlers.on_dispatcher_status {
                        let response = handler(request, msg_id, &rpc, &state).await;
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::DispatcherStatusResponse(response))
                            .await
                        {
                            tracing::error!(
                                "failed to send DispatcherStatusResponse reply: {:?}",
                                e
                            );
                        }
                    } else {
                        tracing::warn!(
                            "received DispatcherStatusRequest but no handler registered"
                        );
                    }
                }
                WireMessage::DispatcherStatusResponse(res) => {
                    tracing::debug!(
                        "received DispatcherStatusResponse (unexpected on server): {res:?}"
                    );
                }
                WireMessage::DeviceDisconnectionRequest(request) => {
                    if let Some(handler) = &handlers.on_device_disconnection {
                        let response = handler(request, msg_id, &rpc, &state).await;
                        if let Err(e) = rpc
                            .reply(msg_id, WireMessage::DeviceDisconnectionResponse(response))
                            .await
                        {
                            tracing::error!(
                                "failed to send DeviceDisconnectionResponse reply: {:?}",
                                e
                            );
                        }
                    } else {
                        tracing::warn!(
                            "received DeviceDisconnectionRequest but no handler registered"
                        );
                    }
                }
                WireMessage::DeviceDisconnectionResponse(res) => {
                    tracing::debug!(
                        "received DeviceDisconnectionResponse (unexpected on server): {res:?}"
                    );
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
                            let acceptor = self.acceptor.clone();

                            let handlers = handlers.clone();
                            let state = state.clone();
                            let buffer_size = self.buffer_size;
                            tokio::spawn(async move {
                                match acceptor.accept(stream).await {
                                    Ok(tls_stream) => {
                                        Self::handle_connection(handlers, state, tls_stream, buffer_size).await;
                                    }
                                    Err(err) => {
                                        tracing::error!(%addr, "TLS handshake failed: {:?}", err);
                                    }
                                }
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
