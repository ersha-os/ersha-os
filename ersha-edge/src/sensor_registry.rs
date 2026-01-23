use crate::Error;
use crate::SensorCapability;
use crate::SensorId;
use crate::SensorMetricKind;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

pub static SENSOR_REGISTRY: Mutex<CriticalSectionRawMutex, SensorRegistry> =
    Mutex::new(SensorRegistry::new());

pub const MAX_SENSORS: usize = 128;

pub struct SensorRegistry {
    next_id: SensorId,
    sensors: [Option<SensorMetricKind>; MAX_SENSORS],
}

impl SensorRegistry {
    const fn new() -> Self {
        Self {
            next_id: 0,
            sensors: [None; MAX_SENSORS],
        }
    }

    fn register(&mut self, metric: SensorMetricKind) -> Option<SensorId> {
        if self.next_id as usize >= MAX_SENSORS {
            return None;
        }

        let id = self.next_id;
        self.sensors[id as usize] = Some(metric);
        self.next_id += 1;
        Some(id)
    }

    pub fn capabilities(&self) -> impl Iterator<Item = SensorCapability> + '_ {
        self.sensors.iter().enumerate().filter_map(|(id, metric)| {
            metric.map(|m| SensorCapability {
                sensor_id: id as u8,
                metric: m,
            })
        })
    }
}

pub async fn register_sensor(metric: SensorMetricKind) -> Result<SensorId, Error> {
    let mut reg = SENSOR_REGISTRY.lock().await;

    reg.register(metric).ok_or(Error::TooManySensors)
}
