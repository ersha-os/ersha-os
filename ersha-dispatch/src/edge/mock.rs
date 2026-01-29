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
        center: H3Cell,
    ) -> Self {
        let cells = generate_nearby_cells(center, device_count);
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

/// Generate H3 resolution-10 cells near a center location.
///
/// Creates `count` unique cells by adding random lat/lng offsets within ~0.05°
/// (~5.5 km at Ethiopian latitudes) of the center, giving an ~11 km diameter
/// coverage area per dispatcher.
fn generate_nearby_cells(center: H3Cell, count: usize) -> Vec<H3Cell> {
    let center_cell = h3o::CellIndex::try_from(center.0).expect("valid H3 cell");
    let center_ll = LatLng::from(center_cell);
    let center_lat = center_ll.lat();
    let center_lng = center_ll.lng();

    let mut rng = rand::rng();
    let radius = 0.05_f64; // ~5.5 km at Ethiopian latitudes

    let mut cells = Vec::with_capacity(count);
    let mut seen = std::collections::HashSet::new();

    // Always include the center cell itself
    seen.insert(center.0);
    cells.push(center);

    let max_attempts = count * 10;
    let mut attempts = 0;

    while cells.len() < count && attempts < max_attempts {
        attempts += 1;

        let lat_offset: f64 = rng.random_range(-radius..radius);
        let lng_offset: f64 = rng.random_range(-radius..radius);

        let lat = center_lat + lat_offset;
        let lng = center_lng + lng_offset;

        let Ok(ll) = LatLng::new(lat, lng) else {
            continue;
        };
        let cell = ll.to_cell(Resolution::Ten);
        let cell_u64 = u64::from(cell);

        if seen.insert(cell_u64) {
            cells.push(H3Cell(cell_u64));
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
