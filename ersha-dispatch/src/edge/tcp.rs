use async_trait::async_trait;
use ordered_float::NotNan;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{Span, error, field, info, instrument, warn};
use ulid::Ulid;

use super::{EdgeData, EdgeReceiver};
use crate::state::DispatcherState;
use ersha_core::{
    DeviceId, DisconnectionReason, DispatcherId, H3Cell, Percentage, ReadingId, SensorId,
    SensorReading,
};
use ersha_edge::{
    ReadingPacket,
    transport::{Msg, MsgType, PACKET_PREAMBLE},
};

#[derive(Debug, thiserror::Error)]
pub enum EdgeConnectionError {
    #[error("Handshake failed: expected HELLO, got {0:?}")]
    HandshakeMismatch([u8; 5]),

    #[error("Postcard deserialization failed: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid packet preamble: {0:#010X}")]
    InvalidPreamble(u16),

    #[error("Internal data channel closed")]
    ChannelClosed,
}

pub struct TcpEdgeReceiver {
    addr: SocketAddr,
    dispatcher_id: DispatcherId,
    state: DispatcherState,
}

impl TcpEdgeReceiver {
    pub fn new(addr: SocketAddr, dispatcher_id: DispatcherId, state: DispatcherState) -> Self {
        Self {
            addr,
            dispatcher_id,
            state,
        }
    }
}

#[async_trait]
impl EdgeReceiver for TcpEdgeReceiver {
    type Error = io::Error;

    async fn start(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<EdgeData>, Self::Error> {
        let (tx, rx) = mpsc::channel(100);
        let addr = self.addr;

        let listener = TcpListener::bind(addr).await?;
        info!(%addr, "TCP edge receiver started");

        tokio::spawn(run_server_loop(
            listener,
            tx,
            cancel,
            self.dispatcher_id,
            self.state.clone(),
        ));

        Ok(rx)
    }
}

#[instrument(name = "server_loop", skip_all, fields(?dispatcher_id))]
async fn run_server_loop(
    listener: TcpListener,
    tx: mpsc::Sender<EdgeData>,
    cancel: CancellationToken,
    dispatcher_id: DispatcherId,
    state: DispatcherState,
) {
    info!("TCP edge receiver server started");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Closing TCP edge receiver server");
                break;
            }
            client = listener.accept() => {
                match client {
                    Ok((stream, addr)) => {
                        info!(%addr, "Client connected");

                        let cancel = cancel.clone();
                        let tx = tx.clone();
                        let dispatcher_id = dispatcher_id;
                        let state = state.clone();

                        tokio::spawn(async move {
                            async move {
                                if let Err(e) =
                                    handle_edge_device(stream, tx, cancel, dispatcher_id, state)
                                        .await
                                {
                                    error!(error = %e, "Connection closed with error");
                                }
                            }
                            .await;
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to accept connection");
                        if is_transient_error(&e) {
                             sleep(Duration::from_millis(100)).await;
                        } else {
                             break;
                        }
                    }
                }
            }
        }
    }
}

fn is_transient_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(
        e.kind(),
        ConnectionRefused | ConnectionAborted | ConnectionReset | OutOfMemory | Other
    )
}

#[instrument(
    name = "edge_handler", skip(stream, tx, cancel, state), fields(device_id = field::Empty, ?dispatcher_id)
)]
async fn handle_edge_device(
    mut stream: TcpStream,
    tx: mpsc::Sender<EdgeData>,
    cancel: CancellationToken,
    dispatcher_id: DispatcherId,
    state: DispatcherState,
) -> Result<(), EdgeConnectionError> {
    let mut hello = [0u8; 5];
    stream.read_exact(&mut hello).await?;

    if &hello != b"HELLO" {
        return Err(EdgeConnectionError::HandshakeMismatch(hello));
    }

    let mut location = [0u8; 8];
    stream.read_exact(&mut location).await?;
    let location_raw = u64::from_be_bytes(location);

    let device_ulid = Ulid::new();

    Span::current().record("device_id", field::display(&device_ulid));

    let device_id = DeviceId(device_ulid);

    stream.write_all(&device_ulid.0.to_be_bytes()).await?;
    info!("Handshake complete, device ID assigned");

    // Track device connection
    state.device_connected(device_id).await;

    let mut buf: Vec<u8> = Vec::with_capacity(128);
    let mut tmp = [0u8; 256];
    let mut disconnection_reason = DisconnectionReason::GracefulClose;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Shutdown signal received");
                break;
            }
            read = stream.read(&mut tmp) => {
                let n = match read {
                    Ok(0) => {
                        info!("Device closed connection");
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        disconnection_reason = DisconnectionReason::Error(e.to_string().into());
                        state.device_disconnected(device_id, disconnection_reason).await;
                        return Err(EdgeConnectionError::Io(e));
                    }
                };

                buf.extend_from_slice(&tmp[..n]);

                while !buf.is_empty() {
                    let (msg, rest) = match postcard::take_from_bytes::<Msg>(&buf) {
                        Ok(v) => v,
                        Err(postcard::Error::DeserializeUnexpectedEnd) => break,
                        Err(e) => {
                            disconnection_reason = DisconnectionReason::Error(e.to_string().into());
                            state.device_disconnected(device_id, disconnection_reason).await;
                            return Err(EdgeConnectionError::Postcard(e));
                        }
                    };

                    if msg.preamble != PACKET_PREAMBLE {
                        disconnection_reason = DisconnectionReason::Error("Invalid preamble".into());
                        state.device_disconnected(device_id, disconnection_reason).await;
                        return Err(EdgeConnectionError::InvalidPreamble(msg.preamble));
                    }

                    match msg.msg_type {
                        MsgType::Reading => {
                            let packet: ReadingPacket = postcard::from_bytes(msg.payload)
                                .map_err(|e| {
                                    warn!(error = %e, "Malformed payload for ReadingPacket");
                                    e
                                })?;

                            let reading = EdgeData::Reading(SensorReading {
                                id: ReadingId(Ulid::new()),
                                device_id,
                                dispatcher_id,
                                metric: convert_metric(packet.metric),
                                location: H3Cell(location_raw),
                                confidence: Percentage(100),
                                timestamp: jiff::Timestamp::now(),
                                sensor_id: SensorId(Ulid(packet.sensor_id)),
                            });

                            if tx.send(reading).await.is_err() {
                                error!("Internal dispatcher channel closed");
                                disconnection_reason = DisconnectionReason::Error("Channel closed".into());
                                state.device_disconnected(device_id, disconnection_reason).await;
                                return Err(EdgeConnectionError::ChannelClosed);
                            }
                        }
                    };

                    buf = rest.to_vec();
                }
            }
        }
    }

    // Track device disconnection (for graceful close or shutdown)
    state
        .device_disconnected(device_id, disconnection_reason)
        .await;

    Ok(())
}

fn convert_metric(source: ersha_edge::SensorMetric) -> ersha_core::SensorMetric {
    match source {
        ersha_edge::SensorMetric::SoilMoisture(v) => ersha_core::SensorMetric::SoilMoisture {
            value: ersha_core::Percentage(v),
        },
        ersha_edge::SensorMetric::SoilTemp(v) => ersha_core::SensorMetric::SoilTemp {
            value: NotNan::new(v as f64 / 100.0).unwrap(),
        },
        ersha_edge::SensorMetric::AirTemp(v) => ersha_core::SensorMetric::AirTemp {
            value: NotNan::new(v as f64 / 100.0).unwrap(),
        },
        ersha_edge::SensorMetric::Humidity(v) => ersha_core::SensorMetric::Humidity {
            value: ersha_core::Percentage(v),
        },
        ersha_edge::SensorMetric::Rainfall(v) => ersha_core::SensorMetric::Rainfall {
            value: NotNan::new(v as f64 / 100.0).unwrap(),
        },
    }
}
