use crate::{DeviceId, Error, H3Cell, ReadingPacket};

pub mod wifi;
pub use wifi::*;

use serde::Deserialize;
use serde::Serialize;

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
    fn provision(&mut self, location: H3Cell) -> impl Future<Output = Result<DeviceId, Error>>;

    /// Send a single sensor reading
    fn send_reading(&mut self, packet: &ReadingPacket) -> impl Future<Output = Result<(), Error>>;
}
