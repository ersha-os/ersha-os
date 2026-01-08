use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

use crate::storage::models::{StorageState, StoredDeviceStatus, StoredSensorReading};
use crate::storage::Storage;

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
}
