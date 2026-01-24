#![no_std]


mod engine;
pub use engine::*;

mod transport;
pub use transport::*;

mod sensor;

use core::mem::MaybeUninit;

use defmt::Format;
use serde::{Deserialize, Serialize};

pub type DeviceId = u32;
pub type SensorId = u8;
pub type ReadingId = u16;

#[derive(Serialize, Deserialize, Format, Clone, Copy)]
pub struct SensorCapability {
    pub sensor_id: SensorId,
    pub metric: SensorMetricKind,
}

#[derive(Serialize, Deserialize, Format, Clone, Copy)]
pub enum SensorMetricKind {
    SoilMoisture,
    SoilTemp,
    AirTemp,
    Humidity,
    Rainfall,
}

#[derive(Serialize, Deserialize, Format)]
pub struct ReadingPacket {
    pub device_id: DeviceId,
    pub sensor_id: SensorId,
    pub reading_id: ReadingId,
    pub metric: SensorMetric,
}

#[derive(Clone, Format)]
pub struct TaggedReading {
    pub sensor_id: SensorId,
    pub metric: SensorMetric,
}

#[derive(Serialize, Deserialize, Debug, Format)]
pub struct UplinkPacket {
    pub seq: u8,
    pub sensor_id: u8,
    pub metric: SensorMetric,
}

#[derive(Debug, Format)]
pub enum Error {
    UnableToSend,
    SerializationFailed,
    ServerNotFound,
    TooManySensors,
}

#[macro_export]
macro_rules! sensor_task {
    ($task_name:ident, $sensor_ty:ty, $metric_kind:expr) => {
        #[embassy_executor::task]
        async fn $task_name(sensor: &'static $sensor_ty) -> ! {
            use defmt::error;
            use embassy_time::Timer;

            let sender = $crate::sender();


            loop {
                let config = sensor.config();

                match sensor.read().await {
                    Ok(reading) => {
                        let reading = $crate::TaggedReading {
                            sensor_id,
                            metric: reading.calibrate(config.calibration_offset),
                        };

                        if sender.try_send(reading).is_err() {
                            defmt::warn!("Sensor queue full, dropping reading");
                        };
                    }
                    Err(e) => {
                        error!("Sender Error: {:?}", e);
                    }
                }

                Timer::after(config.sampling_rate).await;
            }
        }
    };
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use crate::sensor::{Sensor, SensorConfig, SensorError};

    use super::*;
    use embassy_time::Duration;

    struct MockSoilSensor;

    impl Sensor for MockSoilSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                calibration_offset: 0,
            }
        }

        fn read(&self) -> impl core::future::Future<Output = Result<SensorMetric, SensorError>> {
            async move { Ok(SensorMetric::SoilMoisture(42)) }
        }
    }

    sensor_task!(soil_task, MockSoilSensor, SensorMetricKind::SoilMoisture);

    struct MockAirSensor;

    impl Sensor for MockAirSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                calibration_offset: 0,
            }
        }

        async fn read(&self) -> Result<SensorMetric, SensorError> {
            Err(SensorError::InvalidData)
        }
    }

    sensor_task!(air_task, MockAirSensor, SensorMetricKind::SoilMoisture);

    struct MockRainSensor;

    impl Sensor for MockRainSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                calibration_offset: 0,
            }
        }

        async fn read(&self) -> Result<SensorMetric, SensorError> {
            Err(SensorError::Timeout)
        }
    }

    sensor_task!(rain_task, MockRainSensor, SensorMetricKind::SoilMoisture);
}
