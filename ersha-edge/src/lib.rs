#![no_std]

use defmt::info;
use embassy_executor::{SpawnError, Spawner};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::{Duration, Timer};
use ersha_core::{SensorKind, SensorMetric};
use heapless::Vec;

#[allow(dead_code)]
const SENSOR_PER_DEVICE: usize = 8;

static CHANNEL: Channel<CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE> = Channel::new();

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
    // async fn read(&self) -> Result<SensorMetric, SensorError>;
}

pub async fn start(
    spawner: Spawner,
    sensors: Vec<&'static dyn Sensor, SENSOR_PER_DEVICE>,
) -> Result<(), SpawnError> {
    for sensor in sensors {
        spawner.spawn(sensor_task(sensor, CHANNEL.sender()))?;
    }

    info!("All sensor tasks spawned. Waiting for data...");

    let receiver = CHANNEL.receiver();
    loop {
        let reading = receiver.receive().await;

        match reading {
            SensorMetric::SoilMoisture { value } => {
                info!("LoRaWAN Sending: Soil Moisture {}%", value)
            }
            SensorMetric::AirTemp { value } => info!("LoRaWAN Sending: Air Temp {} C", value),
            _ => info!("LoRaWAN Sending: Other metric"),
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task(pool_size = SENSOR_PER_DEVICE)]
async fn sensor_task(
    sensor: &'static dyn Sensor,
    _sender: Sender<'static, CriticalSectionRawMutex, SensorMetric, SENSOR_PER_DEVICE>,
) -> ! {
    let config = sensor.config();

    loop {
        info!("Reading sensor kind: {:?}", config.kind);

        // match sensor.read().await {
        //     Ok(reading) => {
        //         sender.send(reading).await;
        //     }
        //     Err(e) => {
        //         defmt::error!("Sensor Error: {:?}", e);
        //     }
        // }

        Timer::after(config.sampling_rate).await;
    }
}
