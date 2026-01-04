use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorType {
    Temperature = 0x01,
    Humidity = 0x02,
    Pressure = 0x03,
    Light = 0x04,
    Motion = 0x05,
    Battery = 0x06,
    Accelerometer = 0x07,
}

impl TryFrom<u8> for SensorType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(SensorType::Temperature),
            0x02 => Ok(SensorType::Humidity),
            0x03 => Ok(SensorType::Pressure),
            0x04 => Ok(SensorType::Light),
            0x05 => Ok(SensorType::Motion),
            0x06 => Ok(SensorType::Battery),
            0x07 => Ok(SensorType::Accelerometer),
            _ => Err(ProtocolError::InvalidSensorType(value)),
        }
    }
}
