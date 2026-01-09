use ersha_core::{BatchUploadRequest, BatchUploadResponse};
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpStream;

use crate::{Hello, RpcError, RpcTcp, WireMessage};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Client {
    rpc: RpcTcp,
    timeout: Duration,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Unexpected response type")]
    UnexpectedResponse,
    #[error("Error response: {0}")]
    ErrorResponse(String),
}

impl Client {
    pub fn new(stream: TcpStream) -> Self {
        Self::with_buffer(stream, 1024)
    }

    pub fn with_buffer(stream: TcpStream, buffer: usize) -> Self {
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
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err.message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn batch_upload_request(
        &self,
        request: BatchUploadRequest,
    ) -> Result<BatchUploadResponse, ClientError> {
        let response = self
            .rpc
            .call(WireMessage::BatchUploadRequest(request), self.timeout)
            .await?;

        match response.payload {
            WireMessage::BatchUploadResponse(resp) => Ok(resp),
            WireMessage::Error(err) => Err(ClientError::ErrorResponse(err.message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    pub async fn hello(&self, hello: Hello) -> Result<(), ClientError> {
        self.rpc
            .send(WireMessage::Hello(hello))
            .await
            .map_err(ClientError::from)?;
        Ok(())
    }
}
