mod error;
mod packet;
mod sensor;

use error::ProtocolError;

pub const PACKET_PREAMBLE: u16 = 0xE45A;
pub const PROTOCOL_VERSION: u8 = 0x01;
pub const MAX_PACKET_SIZE: usize = 128;
pub const PREAMBLE_SIZE: usize = 2;
pub const PACKET_HEADER_SIZE: usize = 6;
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - PREAMBLE_SIZE - PACKET_HEADER_SIZE;
