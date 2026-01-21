use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

use crate::storage::{
    CleanupStats, DeviceStatusStorage, SensorReadingsStorage, StorageMaintenance, StorageStats,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageState {
    Pending,
    Uploaded,
}

#[derive(Debug, Clone)]
pub struct StoredSensorReading {
    pub id: ReadingId,
    pub reading: SensorReading,
    pub state: StorageState,
}

#[derive(Debug, Clone)]
pub struct StoredDeviceStatus {
    pub id: StatusId,
    pub status: DeviceStatus,
    pub state: StorageState,
}

/// In-memory storage implementation.
/// This is primarily intended for testing and as a reference
/// implementation of the Storage trait.
#[derive(Clone, Default)]
pub struct MemoryStorage {
    sensor_readings: Arc<Mutex<HashMap<ReadingId, StoredSensorReading>>>,
    device_statuses: Arc<Mutex<HashMap<StatusId, StoredDeviceStatus>>>,
}

/// Error type for MemoryStorage
#[derive(Debug)]
pub enum MemoryStorageError {
    MutexPoisoned(String),
}

impl std::error::Error for MemoryStorageError {}

impl fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryStorageError::MutexPoisoned(msg) => write!(f, "Mutex poisoned: {}", msg),
        }
    }
}

impl<T> From<PoisonError<T>> for MemoryStorageError {
    fn from(err: PoisonError<T>) -> Self {
        MemoryStorageError::MutexPoisoned(err.to_string())
    }
}

#[async_trait]
impl SensorReadingsStorage for MemoryStorage {
    type Error = MemoryStorageError;

    async fn store(&self, reading: SensorReading) -> Result<(), Self::Error> {
        let mut map = self.sensor_readings.lock()?;

        let id = reading.id;
        map.insert(
            id,
            StoredSensorReading {
                id,
                reading,
                state: StorageState::Pending,
            },
        );

        Ok(())
    }

    async fn store_batch(&self, readings: Vec<SensorReading>) -> Result<(), Self::Error> {
        let mut map = self.sensor_readings.lock()?;

        for reading in readings {
            let id = reading.id;
            map.insert(
                id,
                StoredSensorReading {
                    id,
                    reading,
                    state: StorageState::Pending,
                },
            );
        }

        Ok(())
    }

    async fn fetch_pending(&self) -> Result<Vec<SensorReading>, Self::Error> {
        let map = self.sensor_readings.lock()?;

        Ok(map
            .values()
            .filter(|r| r.state == StorageState::Pending)
            .map(|r| r.reading.clone())
            .collect())
    }

    async fn mark_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error> {
        let mut map = self.sensor_readings.lock()?;

        for id in ids {
            if let Some(entry) = map.get_mut(id) {
                entry.state = StorageState::Uploaded;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl DeviceStatusStorage for MemoryStorage {
    type Error = MemoryStorageError;

    async fn store(&self, status: DeviceStatus) -> Result<(), Self::Error> {
        let mut map = self.device_statuses.lock()?;

        let id = status.id;
        map.insert(
            id,
            StoredDeviceStatus {
                id,
                status,
                state: StorageState::Pending,
            },
        );

        Ok(())
    }

    async fn store_batch(&self, statuses: Vec<DeviceStatus>) -> Result<(), Self::Error> {
        let mut map = self.device_statuses.lock()?;

        for status in statuses {
            let id = status.id;
            map.insert(
                id,
                StoredDeviceStatus {
                    id,
                    status,
                    state: StorageState::Pending,
                },
            );
        }

        Ok(())
    }

    async fn fetch_pending(&self) -> Result<Vec<DeviceStatus>, Self::Error> {
        let map = self.device_statuses.lock()?;

        Ok(map
            .values()
            .filter(|s| s.state == StorageState::Pending)
            .map(|s| s.status.clone())
            .collect())
    }

    async fn mark_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error> {
        let mut map = self.device_statuses.lock()?;

        for id in ids {
            if let Some(entry) = map.get_mut(id) {
                entry.state = StorageState::Uploaded;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl StorageMaintenance for MemoryStorage {
    type Error = MemoryStorageError;

    async fn get_stats(&self) -> Result<StorageStats, Self::Error> {
        let sensor_map = self.sensor_readings.lock()?;
        let device_map = self.device_statuses.lock()?;

        let sensor_readings_total = sensor_map.len();
        let sensor_readings_pending = sensor_map
            .values()
            .filter(|r| r.state == StorageState::Pending)
            .count();
        let sensor_readings_uploaded = sensor_readings_total - sensor_readings_pending;

        let device_statuses_total = device_map.len();
        let device_statuses_pending = device_map
            .values()
            .filter(|s| s.state == StorageState::Pending)
            .count();
        let device_statuses_uploaded = device_statuses_total - device_statuses_pending;

        Ok(StorageStats {
            sensor_readings_pending,
            sensor_readings_uploaded,
            sensor_readings_total,
            device_statuses_pending,
            device_statuses_uploaded,
            device_statuses_total,
        })
    }

    async fn cleanup_uploaded(&self, _older_than: Duration) -> Result<CleanupStats, Self::Error> {
        let mut sensor_map = self.sensor_readings.lock()?;
        let mut device_map = self.device_statuses.lock()?;

        // Memory storage: just remove uploaded entries (ignore timestamp)
        let sensor_keys_to_remove: Vec<_> = sensor_map
            .iter()
            .filter(|(_, v)| v.state == StorageState::Uploaded)
            .map(|(k, _)| *k)
            .collect();

        let sensor_readings_deleted = sensor_keys_to_remove.len();
        for key in sensor_keys_to_remove {
            sensor_map.remove(&key);
        }

        let device_keys_to_remove: Vec<_> = device_map
            .iter()
            .filter(|(_, v)| v.state == StorageState::Uploaded)
            .map(|(k, _)| *k)
            .collect();

        let device_statuses_deleted = device_keys_to_remove.len();
        for key in device_keys_to_remove {
            device_map.remove(&key);
        }

        Ok(CleanupStats {
            sensor_readings_deleted,
            device_statuses_deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryStorage, MemoryStorageError};
    use crate::storage::{DeviceStatusStorage, SensorReadingsStorage, StorageMaintenance};
    use ersha_core::*;
    use std::time::Duration;
    use ulid::Ulid;

    fn dummy_reading() -> SensorReading {
        SensorReading {
            id: ReadingId(Ulid::new()),
            device_id: DeviceId(Ulid::new()),
            dispatcher_id: DispatcherId(Ulid::new()),
            metric: SensorMetric::SoilMoisture {
                value: Percentage(42),
            },
            location: H3Cell(123),
            confidence: Percentage(95),
            timestamp: jiff::Timestamp::now(),
            sensor_id: SensorId(Ulid::new()),
        }
    }

    fn dummy_status() -> DeviceStatus {
        DeviceStatus {
            id: StatusId(Ulid::new()),
            device_id: DeviceId(Ulid::new()),
            dispatcher_id: DispatcherId(Ulid::new()),
            battery_percent: Percentage(85),
            uptime_seconds: 3600,
            signal_rssi: -65,
            errors: Box::new([]),
            timestamp: jiff::Timestamp::now(),
            sensor_statuses: Box::new([]),
        }
    }

    #[tokio::test]
    async fn memory_sensor_reading_lifecycle() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        let reading = dummy_reading();
        let reading_id = reading.id;

        SensorReadingsStorage::store(&storage, reading).await?;

        let pending: Vec<SensorReading> = SensorReadingsStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 1);

        SensorReadingsStorage::mark_uploaded(&storage, std::slice::from_ref(&reading_id)).await?;

        let pending: Vec<SensorReading> = SensorReadingsStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn memory_device_status_lifecycle() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        let status = dummy_status();
        let status_id = status.id;

        DeviceStatusStorage::store(&storage, status).await?;

        let pending: Vec<DeviceStatus> = DeviceStatusStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 1);

        DeviceStatusStorage::mark_uploaded(&storage, std::slice::from_ref(&status_id)).await?;

        let pending: Vec<DeviceStatus> = DeviceStatusStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn memory_mixed_events() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        let reading = dummy_reading();
        let status = dummy_status();

        SensorReadingsStorage::store(&storage, reading).await?;
        DeviceStatusStorage::store(&storage, status).await?;

        let pending_readings: Vec<SensorReading> =
            SensorReadingsStorage::fetch_pending(&storage).await?;
        let pending_statuses: Vec<DeviceStatus> =
            DeviceStatusStorage::fetch_pending(&storage).await?;

        assert_eq!(pending_readings.len(), 1);
        assert_eq!(pending_statuses.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn memory_batch_sensor_readings() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        let readings = vec![dummy_reading(), dummy_reading(), dummy_reading()];

        SensorReadingsStorage::store_batch(&storage, readings).await?;

        let pending: Vec<SensorReading> = SensorReadingsStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn memory_batch_device_statuses() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        let statuses = vec![dummy_status(), dummy_status()];

        DeviceStatusStorage::store_batch(&storage, statuses).await?;

        let pending: Vec<DeviceStatus> = DeviceStatusStorage::fetch_pending(&storage).await?;
        assert_eq!(pending.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn memory_get_stats() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        // initial stats should be zero
        let stats = storage.get_stats().await?;
        assert_eq!(stats.sensor_readings_total, 0);
        assert_eq!(stats.device_statuses_total, 0);

        SensorReadingsStorage::store(&storage, dummy_reading()).await?;
        SensorReadingsStorage::store(&storage, dummy_reading()).await?;
        DeviceStatusStorage::store(&storage, dummy_status()).await?;

        let stats = storage.get_stats().await?;
        assert_eq!(stats.sensor_readings_total, 2);
        assert_eq!(stats.sensor_readings_pending, 2);
        assert_eq!(stats.sensor_readings_uploaded, 0);
        assert_eq!(stats.device_statuses_total, 1);
        assert_eq!(stats.device_statuses_pending, 1);
        assert_eq!(stats.device_statuses_uploaded, 0);

        let reading = dummy_reading();
        let reading_id = reading.id;
        SensorReadingsStorage::store(&storage, reading).await?;
        SensorReadingsStorage::mark_uploaded(&storage, std::slice::from_ref(&reading_id)).await?;

        let stats = storage.get_stats().await?;
        assert_eq!(stats.sensor_readings_total, 3);
        assert_eq!(stats.sensor_readings_pending, 2);
        assert_eq!(stats.sensor_readings_uploaded, 1);

        Ok(())
    }

    #[tokio::test]
    async fn memory_cleanup_uploaded() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();

        // create 3 readings, mark 2 as uploaded
        let reading1 = dummy_reading();
        let reading2 = dummy_reading();
        let reading3 = dummy_reading();

        let id1 = reading1.id;
        let id2 = reading2.id;

        SensorReadingsStorage::store(&storage, reading1).await?;
        SensorReadingsStorage::store(&storage, reading2).await?;
        SensorReadingsStorage::store(&storage, reading3).await?;
        DeviceStatusStorage::store(&storage, dummy_status()).await?;

        SensorReadingsStorage::mark_uploaded(&storage, &[id1, id2][..]).await?;

        let stats_before = storage.get_stats().await?;
        assert_eq!(stats_before.sensor_readings_total, 3);
        assert_eq!(stats_before.sensor_readings_uploaded, 2);

        // cleanup uploaded (memory ignores duration, deletes all uploaded)
        let cleanup = storage.cleanup_uploaded(Duration::ZERO).await?;
        assert_eq!(cleanup.sensor_readings_deleted, 2);
        assert_eq!(cleanup.device_statuses_deleted, 0); // Not uploaded

        // after cleanup
        let stats_after = storage.get_stats().await?;
        assert_eq!(stats_after.sensor_readings_total, 1); // Only pending remains
        assert_eq!(stats_after.sensor_readings_pending, 1);
        assert_eq!(stats_after.sensor_readings_uploaded, 0);

        Ok(())
    }

    #[tokio::test]
    async fn memory_zero_duration_cleanup() -> Result<(), MemoryStorageError> {
        let storage: MemoryStorage = MemoryStorage::default();
        let reading = dummy_reading();
        let reading_id = reading.id;
        SensorReadingsStorage::store(&storage, reading).await?;
        SensorReadingsStorage::mark_uploaded(&storage, std::slice::from_ref(&reading_id)).await?;

        // memory backend should also delete all uploaded with zero duration
        let cleanup = storage.cleanup_uploaded(Duration::ZERO).await?;
        assert_eq!(cleanup.sensor_readings_deleted, 1);
        assert_eq!(cleanup.device_statuses_deleted, 0);

        // verify data is deleted
        let stats = storage.get_stats().await?;
        assert_eq!(stats.sensor_readings_total, 0);
        assert_eq!(stats.sensor_readings_uploaded, 0);

        Ok(())
    }
}
