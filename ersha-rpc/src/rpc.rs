use dashmap::DashMap;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use crate::{Envelope, MessageId, WireMessage, read_frame, write_frame};

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("send error: {0}")]
    SendError(#[from] mpsc::error::SendError<Envelope>),
    #[error("response channel closed: {0}")]
    ChannelClosed(#[from] oneshot::error::RecvError),
    #[error("timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
}

pub struct RpcTcp {
    tx: mpsc::Sender<Envelope>,
    rx: mpsc::Receiver<Envelope>,
    pending: Arc<DashMap<MessageId, oneshot::Sender<Envelope>>>,
}

impl RpcTcp {
    pub fn new(stream: TcpStream, buffer: usize) -> Self {
        let (reader, writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);

        let (tx_out, mut rx_out) = mpsc::channel::<Envelope>(buffer);
        let (tx_in, rx_in) = mpsc::channel::<Envelope>(buffer);

        let pending: Arc<DashMap<MessageId, oneshot::Sender<Envelope>>> = Arc::new(DashMap::new());

        tokio::spawn(async move {
            while let Some(msg) = rx_out.recv().await {
                if let Err(e) = write_frame(&mut writer, &msg).await {
                    tracing::error!("writer error: {:?}", e);
                    break;
                }
                tracing::info!("wrote message: {msg:?}");
            }
        });

        let pending_clone = pending.clone();
        tokio::spawn(async move {
            loop {
                let msg = match read_frame(&mut reader).await {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!("reader error: {:?}", e);
                        break;
                    }
                };

                tracing::info!("read message: {msg:?}");

                if let Some(reply_to) = msg.reply_to {
                    if let Some((_, tx)) = pending_clone.remove(&reply_to) {
                        let _ = tx.send(msg);
                        continue;
                    }
                    tracing::warn!("no waiter found for reply");
                }

                if tx_in.send(msg).await.is_err() {
                    break;
                }
            }
        });

        Self {
            tx: tx_out,
            rx: rx_in,
            pending,
        }
    }

    pub async fn send(&self, payload: WireMessage) -> Result<MessageId, RpcError> {
        let msg_id = MessageId::new();
        let env = Envelope {
            msg_id,
            reply_to: None,
            payload,
        };

        self.tx.send(env).await?;

        Ok(msg_id)
    }

    pub async fn recv(&mut self) -> Option<Envelope> {
        self.rx.recv().await
    }

    pub async fn call(
        &self,
        payload: WireMessage,
        timeout: Duration,
    ) -> Result<Envelope, RpcError> {
        let msg_id = MessageId::new();
        let (tx_wait, rx_wait) = oneshot::channel();

        self.pending.insert(msg_id, tx_wait);

        let env = Envelope {
            msg_id,
            reply_to: None,
            payload,
        };

        if let Err(e) = self.tx.send(env).await {
            self.pending.remove(&msg_id);
            return Err(RpcError::SendError(e));
        }

        match tokio::time::timeout(timeout, rx_wait).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(closed)) => Err(RpcError::ChannelClosed(closed)),
            Err(elapsed) => {
                self.pending.remove(&msg_id);
                Err(RpcError::Timeout(elapsed))
            }
        }
    }

    pub async fn reply(
        &self,
        request_msg_id: MessageId,
        payload: WireMessage,
    ) -> Result<MessageId, RpcError> {
        let msg_id = MessageId::new();
        let env = Envelope {
            msg_id,
            reply_to: Some(request_msg_id),
            payload,
        };

        self.tx.send(env).await?;

        Ok(msg_id)
    }
}
