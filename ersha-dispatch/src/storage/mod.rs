pub mod memory;
pub mod models;
pub mod sqlite;

use async_trait::async_trait;
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};
use std::time::Duration;

/// Storage abstraction for the dispatcher.
/// This trait defines the minimum set of operations required
/// to persist events locally and track their upload state.
#[async_trait]
pub trait Storage: Send + Sync + 'static {
    /// Error type specific to this storage implementation
    type Error: std::error::Error + Send + Sync + 'static;

    /// Store a sensor reading event as pending.
    async fn store_sensor_reading(&self, reading: SensorReading) -> Result<(), Self::Error>;

    /// Store a device status event as pending.
    async fn store_device_status(&self, status: DeviceStatus) -> Result<(), Self::Error>;

    /// Store multiple sensor readings in a batch (more efficient).
    async fn store_sensor_readings_batch(
        &self,
        readings: Vec<SensorReading>,
    ) -> Result<(), Self::Error>;

    /// Store multiple device statuses in a batch (more efficient).
    async fn store_device_statuses_batch(
        &self,
        statuses: Vec<DeviceStatus>,
    ) -> Result<(), Self::Error>;

    /// Fetch all pending sensor readings.
    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error>;

    /// Fetch all pending device status events.
    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error>;

    /// Mark sensor readings as successfully uploaded.
    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error>;

    /// Mark device status events as successfully uploaded.
    async fn mark_device_statuses_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error>;

    /// Get statistics about stored data.
    async fn get_stats(&self) -> Result<StorageStats, Self::Error>;

    /// Clean up uploaded data older than the specified duration.
    async fn cleanup_uploaded(&self, older_than: Duration) -> Result<CleanupStats, Self::Error>;
}

/// Statistics about stored data.
#[derive(Debug, Clone, Copy, Default)]
pub struct StorageStats {
    /// Number of pending sensor readings.
    pub sensor_readings_pending: usize,
    /// Number of uploaded sensor readings.
    pub sensor_readings_uploaded: usize,
    /// Total number of sensor readings.
    pub sensor_readings_total: usize,
    /// Number of pending device statuses.
    pub device_statuses_pending: usize,
    /// Number of uploaded device statuses.
    pub device_statuses_uploaded: usize,
    /// Total number of device statuses.
    pub device_statuses_total: usize,
}

/// Statistics about cleanup operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupStats {
    /// Number of sensor readings deleted.
    pub sensor_readings_deleted: usize,
    /// Number of device statuses deleted.
    pub device_statuses_deleted: usize,
}
