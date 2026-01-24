use embassy_time::Duration;

use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(defmt::Format)]
pub enum SensorError {
    Timeout,
    InvalidData,
}

// TODO: consolidate with ersha-core::SensorMetric after
// resolveing no_std issues.
#[derive(Serialize, Deserialize, Debug, Clone, Format)]
pub enum SensorMetric {
    /// Percentage 0-100 (1 byte in Postcard)
    SoilMoisture(u8),
    /// Degrees Celsius scaled by 100 (e.g., 25.43 -> 2543).
    /// Fits in 2 bytes instead of 4.
    SoilTemp(i16),
    AirTemp(i16),
    Humidity(u8),
    /// Rainfall in mm scaled by 100.
    Rainfall(u16),
}

impl SensorMetric {
    pub fn calibrate(self, offset: i16) -> Self {
        match self {
            SensorMetric::SoilMoisture(val) => {
                let v = val as i16 + offset;
                let v = v.clamp(0, 100);
                SensorMetric::SoilMoisture(v as u8)
            }

            SensorMetric::Humidity(val) => {
                let v = val as i16 + offset;
                let v = v.clamp(0, 100);
                SensorMetric::Humidity(v as u8)
            }

            SensorMetric::SoilTemp(val) => SensorMetric::SoilTemp(val.saturating_add(offset)),

            SensorMetric::AirTemp(val) => SensorMetric::AirTemp(val.saturating_add(offset)),

            SensorMetric::Rainfall(val) => {
                let v = val as i32 + offset as i32;
                let v = v.clamp(0, u16::MAX as i32);
                SensorMetric::Rainfall(v as u16)
            }
        }
    }
}

pub struct SensorConfig {
    pub sampling_rate: Duration,
}

pub trait Sensor {
    fn config(&self) -> SensorConfig;
    fn read(&self) -> impl Future<Output = Result<SensorMetric, SensorError>>;
}
