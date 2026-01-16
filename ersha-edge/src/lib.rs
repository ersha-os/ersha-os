//! ```ignore
//! #![no_std]
//!
//! use core::future::Future;
//! use embassy_executor::Executor;
//! use embassy_executor::Spawner;
//! use embassy_time::{Duration, Timer};
//! use ersha_edge::{Sensor, SensorConfig, SensorError, SensorMetric, sensor_task};
//! use defmt::info;
//!
//! pub struct MySoilSensor;
//!
//! impl Sensor for MySoilSensor {
//!
//!     fn config(&self) -> SensorConfig {
//!         SensorConfig {
//!             kind: ersha_core::SensorKind::SoilMoisture,
//!             sampling_rate: Duration::from_millis(500),
//!             calibration_offset: 0.0,
//!         }
//!     }
//!
//!     async fn read(&self) -> Self::ReadFuture<'_> {
//!         Ok(SensorMetric::SoilMoisture { value: ersha_core::Percentage(42) })
//!     }
//! }
//!
//! // Generate an embassy task for the sensor
//! sensor_task!(soil_task, MySoilSensor);
//!
//! // Example of spawning and running the executor
//! #[embassy_executor::main]
//! async fn main(spawner: Spawner) {
//!     static SENSOR: MySoilSensor = MySoilSensor;
//!
//!     // Spawn the sensor task
//!     spawner.spawn(soil_task(&SENSOR)).unwrap();
//!
//!     // Start the library's central processing loop
//!     ersha_edge::start().await;
//! }
//! ```

#![no_std]
#![allow(dead_code)]

use defmt::info;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::{Duration, Timer};
use ersha_core::{SensorKind, SensorMetric};

#[allow(dead_code)]
const SENSOR_PER_DEVICE: usize = 8;

static CHANNEL: Channel<CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE> = Channel::new();

fn sender() -> Sender<'static, CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE> {
    CHANNEL.sender()
}

pub struct SensorConfig {
    pub kind: SensorKind,
    pub sampling_rate: Duration,
    pub calibration_offset: f32,
}

#[derive(defmt::Format)]
pub enum SensorError {
    Timeout,
    InvalidData,
}

pub trait Sensor: Send + Sync {
    fn config(&self) -> SensorConfig;
    fn read(&self) -> impl Future<Output = Result<SensorMetric, SensorError>>;
}

pub async fn start() {
    let receiver = CHANNEL.receiver();

    loop {
        match receiver.receive().await {
            SensorMetric::SoilMoisture { value } => {
                info!("LoRaWAN Sending: Soil Moisture {}%", value)
            }
            SensorMetric::AirTemp { value } => info!("LoRaWAN Sending: Air Temp {} C", value),
            _ => info!("LoRaWAN Sending: Other metric"),
        }

        Timer::after_millis(100).await;
    }
}

#[macro_export]
macro_rules! sensor_task {
    ($task_name:ident, $sensor_ty:ty) => {
        #[embassy_executor::task]
        async fn $task_name(sensor: &'static $sensor_ty) -> ! {
            let sender = $crate::sender();

            loop {
                let config = sensor.config();
                info!("Reading sensor kind: {:?}", config.kind);

                match sensor.read().await {
                    Ok(reading) => {
                        sender.send(reading).await;
                    }
                    Err(e) => {
                        defmt::error!("Sender Error: {:?}", e);
                    }
                }

                Timer::after(config.sampling_rate).await;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_time::Duration;

    struct MockSoilSensor;

    impl Sensor for MockSoilSensor {
        fn config(&self) -> SensorConfig {
            SensorConfig {
                kind: SensorKind::SoilMoisture,
                sampling_rate: Duration::from_millis(10),
                calibration_offset: 0.0,
            }
        }

        fn read(&self) -> impl core::future::Future<Output = Result<SensorMetric, SensorError>> {
            async move {
                Ok(SensorMetric::SoilMoisture {
                    value: ersha_core::Percentage(42),
                })
            }
        }
    }

    struct MockAirSensor;

    sensor_task!(soil_task, MockSoilSensor);
}
