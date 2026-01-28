use embassy_net::{
    IpAddress, IpEndpoint, Stack,
    tcp::{State, TcpSocket},
};

use crate::{DeviceId, Error, H3Cell, ReadingPacket};

use super::MAX_PACKET_SIZE;
use super::Transport;

use super::MAX_PAYLOAD_SIZE;
use super::Msg;
use super::MsgType;
use super::PACKET_PREAMBLE;
use super::PROTOCOL_VERSION;

const SERVER_ADDR: IpEndpoint = IpEndpoint {
    addr: IpAddress::v4(10, 46, 238, 14),
    port: 9001,
};

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
    async fn provision(&mut self, location: H3Cell) -> Result<DeviceId, Error> {
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

        // TODO: use a proper frame
        self.socket
            .write(&location.to_be_bytes())
            .await
            .map_err(|_| Error::UnableToSend)?;

        let mut buf = [0u8; 16];
        read_exact(&mut self.socket, &mut buf)
            .await
            .map_err(|_| Error::UnableToSend)?;

        let id = u128::from_be_bytes(buf);
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
