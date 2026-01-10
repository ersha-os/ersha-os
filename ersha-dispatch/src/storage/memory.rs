use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

use crate::storage::models::{StorageState, StoredDeviceStatus, StoredSensorReading};
use crate::storage::{CleanupStats, Storage, StorageStats};

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
impl Storage for MemoryStorage {
    type Error = MemoryStorageError;

    async fn store_sensor_reading(&self, reading: SensorReading) -> Result<(), Self::Error> {
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

    async fn store_device_status(&self, status: DeviceStatus) -> Result<(), Self::Error> {
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

    async fn store_sensor_readings_batch(
        &self,
        readings: Vec<SensorReading>,
    ) -> Result<(), Self::Error> {
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

    async fn store_device_statuses_batch(
        &self,
        statuses: Vec<DeviceStatus>,
    ) -> Result<(), Self::Error> {
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

    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error> {
        let map = self.sensor_readings.lock()?;

        Ok(map
            .values()
            .filter(|r| r.state == StorageState::Pending)
            .map(|r| r.reading.clone())
            .collect())
    }

    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error> {
        let map = self.device_statuses.lock()?;

        Ok(map
            .values()
            .filter(|s| s.state == StorageState::Pending)
            .map(|s| s.status.clone())
            .collect())
    }

    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error> {
        let mut map = self.sensor_readings.lock()?;

        for id in ids {
            if let Some(entry) = map.get_mut(id) {
                entry.state = StorageState::Uploaded;
            }
        }

        Ok(())
    }

    async fn mark_device_statuses_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error> {
        let mut map = self.device_statuses.lock()?;

        for id in ids {
            if let Some(entry) = map.get_mut(id) {
                entry.state = StorageState::Uploaded;
            }
        }

        Ok(())
    }

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
