use async_trait::async_trait;
use ersha_core::{
    SensorReading,
    DeviceStatus,
    ReadingId,
    StatusId,
};

/// storage abstraction for the dispatcher.
/// this trait defines the minimum set of operations required
/// to persist events locally and track their upload state.
#[async_trait]
pub trait Storage: Send + Sync {
    /// store a sensor reading event as pending.
    async fn store_sensor_reading(
        &self,
        reading: SensorReading,
    ) -> Result<(), StorageError>;

    /// store a device status event as pending.
    async fn store_device_status(
        &self,
        status: DeviceStatus,
    ) -> Result<(), StorageError>;

    /// fetch all pending sensor readings.
    async fn fetch_pending_sensor_readings(
        &self,
    ) -> Result<Vec<SensorReading>, StorageError>;

    /// fetch all pending device status events.
    async fn fetch_pending_device_statuses(
        &self,
    ) -> Result<Vec<DeviceStatus>, StorageError>;

    /// mark sensor readings as successfully uploaded.
    async fn mark_sensor_readings_uploaded(
        &self,
        ids: &[ReadingId],
    ) -> Result<(), StorageError>;

    /// mark device status events as successfully uploaded.
    async fn mark_device_statuses_uploaded(
        &self,
        ids: &[StatusId],
    ) -> Result<(), StorageError>;
}

#[derive(Debug)]
pub enum StorageError {
    /// generic storage failure
    Internal(String),
}

