use super::{error::ParseResult, error::ProtocolError, *};

// packet structure : preamble(2) + header(6) + payload + crc(2) + postamble(1)

#[derive(Debug, Clone, Copy)]
pub enum PacketType {
    SensorData = 0x01,
    Heartbeat = 0x02,
}

impl TryFrom<u8> for PacketType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PacketType::SensorData),
            0x02 => Ok(PacketType::Heartbeat),
            _ => Err(ProtocolError::InvalidPacketType(value)),
        }
    }
}

pub struct PacketHeader {
    pub version: u8,
    pub packet_type: PacketType,
    pub node_id: u16,
    pub payload_len: u16,
}

impl PacketHeader {
    pub fn new(packet_type: PacketType, node_id: u16, payload_len: u16) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            packet_type: packet_type,
            node_id,
            payload_len,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> ParseResult<Self> {
        if bytes.len() < 6 {
            return Err(ProtocolError::InsufficientData {
                needed: 6,
                available: bytes.len(),
            });
        }

        if bytes[0] != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(bytes[0]));
        }

        Ok(Self {
            version: bytes[0],
            packet_type: PacketType::try_from(bytes[1])?,
            node_id: u16::from_le_bytes([bytes[2], bytes[3]]),
            payload_len: u16::from_le_bytes([bytes[4], bytes[5]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 6] {
        let mut bytes = [0u8; 6];

        bytes[0] = self.version;
        bytes[1] = self.packet_type as u8;

        let node_le: [u8; 2] = self.node_id.to_le_bytes();
        let payload_len_le: [u8; 2] = self.payload_len.to_le_bytes();

        bytes[2] = node_le[0];
        bytes[3] = node_le[1];
        bytes[4] = payload_len_le[0];
        bytes[5] = payload_len_le[1];

        bytes
    }
}

pub struct Packet {
    pub preamble: u16,
    pub header: PacketHeader,
    pub payload: [u8; MAX_PAYLOAD_SIZE],
}

impl Packet {
    pub fn new(packet_type: PacketType, node_id: u16, payload: &[u8]) -> ParseResult<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                max: MAX_PACKET_SIZE,
                size: payload.len(),
            });
        }

        let mut buf = [0u8; MAX_PAYLOAD_SIZE];
        buf[..payload.len()].copy_from_slice(payload);

        Ok(Self {
            preamble: PACKET_PREAMBLE,
            header: PacketHeader::new(packet_type, node_id, payload.len() as u16),
            payload: buf,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> ParseResult<Self> {
        // it should at least have a preamble and a header
        if bytes.len() < PREAMBLE_SIZE + PACKET_HEADER_SIZE {
            return Err(ProtocolError::InsufficientData {
                needed: PREAMBLE_SIZE + PACKET_HEADER_SIZE,
                available: bytes.len(),
            });
        }

        let preamble = u16::from_le_bytes([bytes[0], bytes[1]]);
        if preamble != PACKET_PREAMBLE {
            return Err(ProtocolError::InvalidPreamble(preamble));
        }

        let header =
            PacketHeader::from_bytes(&bytes[PREAMBLE_SIZE..PREAMBLE_SIZE + PACKET_HEADER_SIZE])?;

        let total_len = PREAMBLE_SIZE + PACKET_HEADER_SIZE + header.payload_len as usize;

        if bytes.len() < total_len {
            return Err(ProtocolError::InsufficientData {
                needed: total_len,
                available: bytes.len(),
            });
        }

        let mut payload = [0u8; MAX_PAYLOAD_SIZE];
        payload[..header.payload_len as usize]
            .copy_from_slice(&bytes[PREAMBLE_SIZE + PACKET_HEADER_SIZE..total_len]);

        Ok(Self {
            preamble,
            header,
            payload,
        })
    }

    pub fn to_bytes(&self) -> [u8; MAX_PACKET_SIZE] {
        let mut bytes = [0u8; MAX_PACKET_SIZE];

        let preamble_le = self.preamble.to_le_bytes();
        bytes[0] = preamble_le[0];
        bytes[1] = preamble_le[1];

        let header_bytes = self.header.to_bytes();
        bytes[PREAMBLE_SIZE..PREAMBLE_SIZE + PACKET_HEADER_SIZE].copy_from_slice(&header_bytes);

        let payload_len = self.header.payload_len as usize;
        bytes[PREAMBLE_SIZE + PACKET_HEADER_SIZE..PREAMBLE_SIZE + PACKET_HEADER_SIZE + payload_len]
            .copy_from_slice(&self.payload[..payload_len]);

        bytes
    }
}
