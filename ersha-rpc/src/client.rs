use ersha_core::{
    AlertRequest, AlertResponse, BatchUploadRequest, BatchUploadResponse,
    DeviceDisconnectionRequest, DeviceDisconnectionResponse, DispatcherStatusRequest,
    DispatcherStatusResponse, HelloRequest, HelloResponse,
};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{RpcError, RpcTcp, WireError, WireMessage};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Client {
    rpc: RpcTcp,
    timeout: Duration,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("rpc error: {0}")]
    Rpc(#[from] RpcError),
    #[error("unexpected response type")]
    UnexpectedResponse,
    #[error("error response: {0:?}")]
    ErrorResponse(WireError),
}

impl Client {
    pub fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        Self::with_buffer(stream, 1024)
    }

    pub fn with_buffer<S>(stream: S, buffer: usize) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        Self {
            rpc: RpcTcp::new(stream, buffer),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn ping(&self) -> Result<(), ClientError> {
        let response = self.rpc.call(WireMessage::Ping, self.timeout).await?;

        match response.payload {
            WireMessage::Pong => Ok(()),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn hello(&self, hello: HelloRequest) -> Result<HelloResponse, ClientError> {
        let response = self
            .rpc
            .call(WireMessage::HelloRequest(hello), self.timeout)
            .await?;

        match response.payload {
            WireMessage::HelloResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn batch_upload(
        &self,
        request: BatchUploadRequest,
    ) -> Result<BatchUploadResponse, ClientError> {
        let response = self
            .rpc
            .call(WireMessage::BatchUploadRequest(request), self.timeout)
            .await?;

        match response.payload {
            WireMessage::BatchUploadResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn alert(&self, request: AlertRequest) -> Result<AlertResponse, ClientError> {
        let response = self
            .rpc
            .call(WireMessage::AlertRequest(request), self.timeout)
            .await?;

        match response.payload {
            WireMessage::AlertResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn dispatcher_status(
        &self,
        request: DispatcherStatusRequest,
    ) -> Result<DispatcherStatusResponse, ClientError> {
        let response = self
            .rpc
            .call(WireMessage::DispatcherStatusRequest(request), self.timeout)
            .await?;

        match response.payload {
            WireMessage::DispatcherStatusResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn device_disconnection(
        &self,
        request: DeviceDisconnectionRequest,
    ) -> Result<DeviceDisconnectionResponse, ClientError> {
        let response = self
            .rpc
            .call(
                WireMessage::DeviceDisconnectionRequest(request),
                self.timeout,
            )
            .await?;

        match response.payload {
            WireMessage::DeviceDisconnectionResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }
}
