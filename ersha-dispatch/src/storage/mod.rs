pub mod memory;
pub mod models;
pub mod sqlite;

use async_trait::async_trait;
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

/// Storage abstraction for the dispatcher.
/// This trait defines the minimum set of operations required
/// to persist events locally and track their upload state.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Error type specific to this storage implementation
    type Error: std::error::Error + Send + Sync + 'static;

    /// Store a sensor reading event as pending.
    async fn store_sensor_reading(&self, reading: SensorReading) -> Result<(), Self::Error>;

    /// Store a device status event as pending.
    async fn store_device_status(&self, status: DeviceStatus) -> Result<(), Self::Error>;

    /// Fetch all pending sensor readings.
    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error>;

    /// Fetch all pending device status events.
    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error>;

    /// Mark sensor readings as successfully uploaded.
    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error>;

    /// Mark device status events as successfully uploaded.
    async fn mark_device_statuses_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error>;
}
