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

const SENSOR_PER_DEVICE: usize = 8;

static READING_CHANNEL: Channel<CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE> =
    Channel::new();

fn sender() -> Sender<'static, CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE> {
    READING_CHANNEL.sender()
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

pub trait Sensor {
    fn config(&self) -> SensorConfig;
    fn read(&self) -> impl Future<Output = Result<SensorMetric, SensorError>>;
}

pub enum UplinkError {
    InvalidAuth,
}

pub struct Reading {
    reading_id: u8, // 1 bytes, we will ahve to generate a real id based on the device id and send it to prime
    metric: u8, // maps to SensorMetric, we could also use fport here. https://github.com/lora-rs/lora-rs/blob/85906f076a54f90d4f8b39aa14fd554df5e434a6/lorawan-device/src/nb_device/mod.rs#L72
    sensor_id: u8, // relative to the device_id
    device_id: u32, // 4 bytes, devaddr
    value: u32,
    // we still have 1 more byte to make a full 12 bytes.
}

impl Reading {
    pub const BYTE_LEN: usize = 11;

    pub fn new(reading_id: u8, metric: u8, sensor_id: u8, device_id: u32, value: u32) -> Self {
        Self {
            reading_id,
            metric,
            sensor_id,
            device_id,
            value,
        }
    }

    pub fn to_bytes(&self) -> [u8; Self::BYTE_LEN] {
        let mut buf = [0u8; Self::BYTE_LEN];

        buf[0] = self.reading_id;
        buf[1] = self.metric;
        buf[2] = self.sensor_id;

        buf[3..7].copy_from_slice(&self.device_id.to_le_bytes());

        buf[7..11].copy_from_slice(&self.value.to_le_bytes());

        buf
    }
}

pub trait Uplink {
    fn send(&self, reading: Reading) -> impl Future<Output = Result<(), UplinkError>>;
}

pub struct Engine<C: Uplink> {
    host: C,
    device_id: u32,
}

impl<C> Engine<C>
where
    C: Uplink,
{
    pub fn new(host: C, device_id: u32) -> Self {
        Self { host, device_id }
    }

    pub async fn run(self) {
        let receiver = READING_CHANNEL.receiver();
        let mut reading_id = 0;

        loop {
            match receiver.receive().await {
                SensorMetric::SoilMoisture { value } => {
                    info!("LoRaWAN Sending: Soil Moisture {}%", value);
                    let _ = self
                        .host
                        .send(Reading {
                            value: value.0 as u32,
                            reading_id,
                            metric: 0, // SoilMoisture { value: Percentage },
                            sensor_id: 0x12,
                            device_id: self.device_id,
                        })
                        .await;

                    reading_id += 1;
                }
                SensorMetric::AirTemp { value } => {
                    info!("LoRaWAN Sending: Air Temp {} C", value);

                    let _ = self
                        .host
                        .send(Reading {
                            value: value.into_inner() as u32,
                            reading_id,
                            metric: 2, // AirTemp { value: NotNan<f64> },
                            sensor_id: 0x13,
                            device_id: self.device_id,
                        })
                        .await;

                    reading_id += 1;
                }
                _ => {
                    info!("LoRaWAN Sending: Other metric");
                    todo!("implement other sensor metrics")
                }
            }

            Timer::after_millis(100).await;
        }
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
