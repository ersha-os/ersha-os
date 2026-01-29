use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ersha_core::{
    DeviceError, DeviceId, DeviceStatus, DispatcherId, H3Cell, Percentage, ReadingId, SensorId,
    SensorMetric, SensorReading, SensorState, SensorStatus, StatusId,
};
use h3o::{LatLng, Resolution};
use ordered_float::NotNan;
use rand::Rng;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use ulid::Ulid;

use super::{EdgeData, EdgeReceiver};

/// Information about a mock device needed for registration with ersha-prime.
pub struct MockDeviceInfo {
    pub device_id: DeviceId,
    pub location: H3Cell,
    pub sensor_ids: Vec<SensorId>,
}

/// Mock edge receiver that generates fake sensor data.
pub struct MockEdgeReceiver {
    /// Dispatcher ID to use in generated data.
    dispatcher_id: DispatcherId,
    /// Interval between sensor readings.
    reading_interval: Duration,
    /// Interval between status updates.
    status_interval: Duration,
    /// Pre-generated mock devices.
    devices: Arc<Vec<MockDevice>>,
}

impl MockEdgeReceiver {
    pub fn new(
        dispatcher_id: DispatcherId,
        reading_interval_secs: u64,
        status_interval_secs: u64,
        device_count: usize,
    ) -> Self {
        let cells = generate_ethiopian_cells(device_count);
        let devices = cells.into_iter().map(MockDevice::new).collect();

        Self {
            dispatcher_id,
            reading_interval: Duration::from_secs(reading_interval_secs),
            status_interval: Duration::from_secs(status_interval_secs),
            devices: Arc::new(devices),
        }
    }

    /// Return registration info for all mock devices.
    ///
    /// These are the same devices that will produce readings and statuses
    /// when `start()` is called.
    pub fn device_info(&self) -> Vec<MockDeviceInfo> {
        self.devices
            .iter()
            .map(|d| MockDeviceInfo {
                device_id: d.device_id,
                location: d.location,
                sensor_ids: d.sensor_ids.clone(),
            })
            .collect()
    }
}

/// Generate H3 resolution-10 cells spread across Ethiopia.
///
/// Uses Ethiopia's approximate bounding box (lat 3.4°–14.9°, lng 33°–48°)
/// to create a grid of points, converts each to an H3 cell, deduplicates,
/// and returns the requested count.
fn generate_ethiopian_cells(count: usize) -> Vec<H3Cell> {
    let lat_min = 3.4_f64;
    let lat_max = 14.9_f64;
    let lng_min = 33.0_f64;
    let lng_max = 48.0_f64;

    // Calculate grid dimensions to produce enough unique cells.
    // Use a square-ish grid with some oversampling to account for deduplication.
    let oversample = (count as f64 * 1.5).sqrt().ceil() as usize;
    let rows = oversample;
    let cols = oversample;

    let lat_step = (lat_max - lat_min) / rows as f64;
    let lng_step = (lng_max - lng_min) / cols as f64;

    let mut cells = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for r in 0..rows {
        for c in 0..cols {
            let lat = lat_min + (r as f64 + 0.5) * lat_step;
            let lng = lng_min + (c as f64 + 0.5) * lng_step;

            let ll = LatLng::new(lat, lng).expect("valid lat/lng for Ethiopia");
            let cell = ll.to_cell(Resolution::Ten);
            let cell_u64 = u64::from(cell);

            if seen.insert(cell_u64) {
                cells.push(H3Cell(cell_u64));
                if cells.len() == count {
                    return cells;
                }
            }
        }
    }

    cells
}

/// A simulated device with stable IDs.
struct MockDevice {
    device_id: DeviceId,
    sensor_ids: Vec<SensorId>,
    location: H3Cell,
}

impl MockDevice {
    fn new(location: H3Cell) -> Self {
        Self {
            device_id: DeviceId(Ulid::new()),
            sensor_ids: vec![
                SensorId(Ulid::new()), // SoilMoisture
                SensorId(Ulid::new()), // SoilTemp
                SensorId(Ulid::new()), // AirTemp
                SensorId(Ulid::new()), // Humidity
                SensorId(Ulid::new()), // Rainfall
            ],
            location,
        }
    }

    fn generate_reading(&self, dispatcher_id: DispatcherId) -> SensorReading {
        let mut rng = rand::rng();
        let sensor_idx = rng.random_range(0..self.sensor_ids.len());
        let sensor_id = self.sensor_ids[sensor_idx];

        let metric = match sensor_idx {
            0 => SensorMetric::SoilMoisture {
                value: Percentage(rng.random_range(20..80)),
            },
            1 => SensorMetric::SoilTemp {
                value: NotNan::new(rng.random_range(15.0..35.0)).unwrap(),
            },
            2 => SensorMetric::AirTemp {
                value: NotNan::new(rng.random_range(10.0..40.0)).unwrap(),
            },
            3 => SensorMetric::Humidity {
                value: Percentage(rng.random_range(30..90)),
            },
            _ => SensorMetric::Rainfall {
                value: NotNan::new(rng.random_range(0.0..50.0)).unwrap(),
            },
        };

        SensorReading {
            id: ReadingId(Ulid::new()),
            device_id: self.device_id,
            dispatcher_id,
            metric,
            location: self.location,
            confidence: Percentage(rng.random_range(85..100)),
            timestamp: jiff::Timestamp::now(),
            sensor_id,
        }
    }

    fn generate_status(&self, dispatcher_id: DispatcherId) -> DeviceStatus {
        let mut rng = rand::rng();

        let sensor_statuses: Vec<SensorStatus> = self
            .sensor_ids
            .iter()
            .map(|&sensor_id| SensorStatus {
                sensor_id,
                state: if rng.random_ratio(95, 100) {
                    SensorState::Active
                } else {
                    SensorState::Faulty
                },
                last_reading: Some(jiff::Timestamp::now()),
            })
            .collect();

        let errors: Vec<DeviceError> = if rng.random_ratio(5, 100) {
            vec![DeviceError {
                code: ersha_core::DeviceErrorCode::LowBattery,
                message: Some("Battery below 20%".into()),
            }]
        } else {
            vec![]
        };

        DeviceStatus {
            id: StatusId(Ulid::new()),
            device_id: self.device_id,
            dispatcher_id,
            battery_percent: Percentage(rng.random_range(20..100)),
            uptime_seconds: rng.random_range(3600..86400),
            signal_rssi: rng.random_range(-80..-30),
            errors: errors.into_boxed_slice(),
            timestamp: jiff::Timestamp::now(),
            sensor_statuses: sensor_statuses.into_boxed_slice(),
        }
    }
}

#[async_trait]
impl EdgeReceiver for MockEdgeReceiver {
    type Error = std::convert::Infallible;

    async fn start(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<EdgeData>, Self::Error> {
        let (tx, rx) = mpsc::channel(100);

        let devices = Arc::clone(&self.devices);
        let dispatcher_id = self.dispatcher_id;
        let reading_interval = self.reading_interval;
        let status_interval = self.status_interval;

        info!(
            device_count = devices.len(),
            reading_interval_secs = reading_interval.as_secs(),
            status_interval_secs = status_interval.as_secs(),
            "Starting mock edge receiver"
        );

        // Spawn reading generator task
        let tx_readings = tx.clone();
        let cancel_readings = cancel.clone();
        let devices_for_readings = Arc::clone(&devices);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(reading_interval);

            loop {
                tokio::select! {
                    _ = cancel_readings.cancelled() => {
                        info!("Mock reading generator shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        for device in devices_for_readings.iter() {
                            let reading = device.generate_reading(dispatcher_id);
                            if tx_readings.send(EdgeData::Reading(reading)).await.is_err() {
                                info!("Channel closed, reading generator shutting down");
                                return;
                            }
                        }
                    }
                }
            }
        });

        // Spawn status generator task
        let tx_statuses = tx;
        let cancel_statuses = cancel;
        let devices_for_statuses = devices;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(status_interval);

            loop {
                tokio::select! {
                    _ = cancel_statuses.cancelled() => {
                        info!("Mock status generator shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        for device in devices_for_statuses.iter() {
                            let status = device.generate_status(dispatcher_id);
                            if tx_statuses.send(EdgeData::Status(status)).await.is_err() {
                                info!("Channel closed, status generator shutting down");
                                return;
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}
