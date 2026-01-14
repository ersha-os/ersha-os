use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

/// upload state of a stored event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageState {
    Pending,
    Uploaded,
}

/// stored sensor reading with storage metadata.
#[derive(Debug, Clone)]
pub struct StoredSensorReading {
    pub id: ReadingId,
    pub reading: SensorReading,
    pub state: StorageState,
}

/// stored device status with storage metadata.
#[derive(Debug, Clone)]
pub struct StoredDeviceStatus {
    pub id: StatusId,
    pub status: DeviceStatus,
    pub state: StorageState,
}
