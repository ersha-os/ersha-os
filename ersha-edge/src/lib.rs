#![no_std]

pub mod engine;
pub mod sensor;
pub mod transport;

pub use engine::Engine;
pub use sensor::{Sensor, SensorMetric};
pub use transport::Transport;

use defmt::Format;
use serde::{Deserialize, Serialize};

pub type DeviceId = u32;
pub type SensorId = u8;
pub type ReadingId = u16;

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

#[derive(Debug, Format)]
pub enum Error {
    UnableToSend,
    SerializationFailed,
    ServerNotFound,
    TooManySensors,
}

#[macro_export]
macro_rules! sensor_task {
    ($task_name:ident, $sensor_ty:ty) => {
        #[embassy_executor::task]
        async fn $task_name(sensor: &'static $sensor_ty) -> ! {
            let sender = $crate::engine::sender();
            let config = sensor.config();

            loop {
                match sensor.read().await {
                    Ok(reading) => {
                        let reading = $crate::TaggedReading {
                            sensor_id: config.sensor_id,
                            metric: reading,
                        };

                        if sender.try_send(reading).is_err() {
                            defmt::warn!("Sensor queue full, dropping reading");
                        };
                    }
                    Err(e) => {
                        defmt::error!("Sender Error: {:?}", e);
                    }
                }

                embassy_time::Timer::after(config.sampling_rate).await;
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
                sensor_id: 1,
            }
        }

        fn read(&self) -> impl core::future::Future<Output = Result<SensorMetric, SensorError>> {
            async move { Ok(SensorMetric::SoilMoisture(42)) }
        }
    }

    sensor_task!(soil_task, MockSoilSensor);

    struct MockAirSensor;

    impl Sensor for MockAirSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                sensor_id: 2,
            }
        }

        async fn read(&self) -> Result<SensorMetric, SensorError> {
            Err(SensorError::InvalidData)
        }
    }

    sensor_task!(air_task, MockAirSensor);

    struct MockRainSensor;

    impl Sensor for MockRainSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                sampling_rate: Duration::from_millis(10),
                sensor_id: 3,
            }
        }

        async fn read(&self) -> Result<SensorMetric, SensorError> {
            Err(SensorError::Timeout)
        }
    }

    sensor_task!(rain_task, MockRainSensor);
}
