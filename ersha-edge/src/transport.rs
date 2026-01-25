use crate::DeviceId;
use crate::Error;
use crate::ReadingPacket;

use embassy_net::{
    IpAddress, IpEndpoint, Stack,
    tcp::{State, TcpSocket},
};
use serde::Deserialize;
use serde::Serialize;

const SERVER_ADDR: IpEndpoint = IpEndpoint {
    addr: IpAddress::v4(10, 46, 238, 14),
    port: 9001,
};

pub const PACKET_PREAMBLE: u16 = 0xE45A;
pub const PROTOCOL_VERSION: u8 = 0x01;
pub const MAX_PACKET_SIZE: usize = 128;
pub const PREAMBLE_SIZE: usize = 2;
pub const PACKET_HEADER_SIZE: usize = 6;
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - PREAMBLE_SIZE - PACKET_HEADER_SIZE;

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgType {
    Reading,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Msg<'a> {
    pub preamble: u16,
    pub version: u8,
    pub msg_type: MsgType,
    pub payload: &'a [u8],
}

pub trait Transport {
    /// Called once after network join / connect
    fn provision(&mut self) -> impl Future<Output = Result<DeviceId, Error>>;

    /// Send a single sensor reading
    fn send_reading(&mut self, packet: &ReadingPacket) -> impl Future<Output = Result<(), Error>>;
}

pub struct Wifi<'a> {
    socket: TcpSocket<'a>,
    device_id: Option<DeviceId>,
}

impl<'a> Wifi<'a> {
    pub fn new(stack: Stack<'a>, rx: &'a mut [u8], tx: &'a mut [u8]) -> Self {
        Self {
            socket: TcpSocket::new(stack, rx, tx),
            device_id: None,
        }
    }
}

impl<'a> Transport for Wifi<'a> {
    async fn provision(&mut self) -> Result<DeviceId, Error> {
        if self.socket.state() != State::Established {
            self.socket
                .connect(SERVER_ADDR)
                .await
                .map_err(|_| Error::ServerNotFound)?;
        }

        self.socket
            .write(b"HELLO")
            .await
            .map_err(|_| Error::UnableToSend)?;

        let mut buf = [0u8; 4];
        read_exact(&mut self.socket, &mut buf)
            .await
            .map_err(|_| Error::UnableToSend)?;

        let id = u32::from_be_bytes(buf);
        self.device_id = Some(id);
        Ok(id)
    }

    async fn send_reading(&mut self, packet: &ReadingPacket) -> Result<(), Error> {
        let mut payload_buf = [0u8; MAX_PAYLOAD_SIZE];
        let payload =
            postcard::to_slice(packet, &mut payload_buf).map_err(|_| Error::SerializationFailed)?;

        let msg = Msg {
            preamble: PACKET_PREAMBLE,
            version: PROTOCOL_VERSION,
            msg_type: MsgType::Reading,
            payload,
        };

        let mut msg_buf = [0u8; MAX_PACKET_SIZE];
        let used =
            postcard::to_slice(&msg, &mut msg_buf).map_err(|_| Error::SerializationFailed)?;

        write_all(&mut self.socket, used).await
    }
}

async fn read_exact(socket: &mut TcpSocket<'_>, mut buf: &mut [u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = socket.read(buf).await.map_err(|_| Error::UnableToSend)?;

        if n == 0 {
            return Err(Error::ServerNotFound);
        }

        buf = &mut buf[n..];
    }
    Ok(())
}

async fn write_all(socket: &mut TcpSocket<'_>, mut buf: &[u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = socket.write(buf).await.map_err(|_| Error::UnableToSend)?;
        buf = &buf[n..];
    }
    Ok(())
}
