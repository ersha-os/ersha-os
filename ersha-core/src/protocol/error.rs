pub type ParseResult<T> = core::result::Result<T, ProtocolError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidPreamble(u16),
    CrcMismatch { expected: u16, actual: u16 },
    InsufficientData { needed: usize, available: usize },
    InvalidPacketType(u8),
    InvalidSensorType(u8),
    InvalidDataFormat(u8),
    PayloadTooLarge { size: usize, max: usize },
    IoError(String),
}
