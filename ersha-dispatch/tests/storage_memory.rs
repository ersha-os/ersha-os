use ersha_core::*;
use ersha_dispatch::storage::Storage;
use ersha_dispatch::storage::memory::{MemoryStorage, MemoryStorageError};
use ersha_dispatch::storage::sqlite::{SqliteStorage, SqliteStorageError};
use ulid::Ulid;
use tempfile::NamedTempFile;

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

// Memory storage tests
#[tokio::test]
async fn memory_sensor_reading_lifecycle() -> Result<(), MemoryStorageError> {
    let storage: MemoryStorage = MemoryStorage::default();

    let reading = dummy_reading();
    let reading_id = reading.id;

    storage.store_sensor_reading(reading).await?;

    let pending = storage.fetch_pending_sensor_readings().await?;
    assert_eq!(pending.len(), 1);

    storage.mark_sensor_readings_uploaded(&[reading_id]).await?;

    let pending = storage.fetch_pending_sensor_readings().await?;
    assert_eq!(pending.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn memory_device_status_lifecycle() -> Result<(), MemoryStorageError> {
    let storage: MemoryStorage = MemoryStorage::default();

    let status = dummy_status();
    let status_id = status.id;

    storage.store_device_status(status).await?;

    let pending = storage.fetch_pending_device_statuses().await?;
    assert_eq!(pending.len(), 1);

    storage.mark_device_statuses_uploaded(&[status_id]).await?;

    let pending = storage.fetch_pending_device_statuses().await?;
    assert_eq!(pending.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn memory_mixed_events() -> Result<(), MemoryStorageError> {
    let storage: MemoryStorage = MemoryStorage::default();

    let reading = dummy_reading();
    let status = dummy_status();

    storage.store_sensor_reading(reading).await?;
    storage.store_device_status(status).await?;

    let pending_readings = storage.fetch_pending_sensor_readings().await?;
    let pending_statuses = storage.fetch_pending_device_statuses().await?;

    assert_eq!(pending_readings.len(), 1);
    assert_eq!(pending_statuses.len(), 1);
    
    Ok(())
}

// SQLite storage tests
#[tokio::test]
async fn sqlite_sensor_reading_lifecycle() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    let storage = SqliteStorage::new(db_path).await?;

    let reading = dummy_reading();
    let reading_id = reading.id;

    storage.store_sensor_reading(reading).await?;

    let pending = storage.fetch_pending_sensor_readings().await?;
    assert_eq!(pending.len(), 1);

    storage.mark_sensor_readings_uploaded(&[reading_id]).await?;

    let pending = storage.fetch_pending_sensor_readings().await?;
    assert_eq!(pending.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn sqlite_device_status_lifecycle() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    let storage = SqliteStorage::new(db_path).await?;

    let status = dummy_status();
    let status_id = status.id;

    storage.store_device_status(status).await?;

    let pending = storage.fetch_pending_device_statuses().await?;
    assert_eq!(pending.len(), 1);

    storage.mark_device_statuses_uploaded(&[status_id]).await?;

    let pending = storage.fetch_pending_device_statuses().await?;
    assert_eq!(pending.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn sqlite_mixed_events() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    let storage = SqliteStorage::new(db_path).await?;

    let reading = dummy_reading();
    let status = dummy_status();

    storage.store_sensor_reading(reading).await?;
    storage.store_device_status(status).await?;

    let pending_readings = storage.fetch_pending_sensor_readings().await?;
    let pending_statuses = storage.fetch_pending_device_statuses().await?;

    assert_eq!(pending_readings.len(), 1);
    assert_eq!(pending_statuses.len(), 1);
    
    Ok(())
}

#[tokio::test]
async fn sqlite_persistence_across_instances() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    {
        let storage = SqliteStorage::new(db_path).await?;
        let reading = dummy_reading();
        storage.store_sensor_reading(reading).await?;
    }
    
    {
        let storage = SqliteStorage::new(db_path).await?;
        let pending = storage.fetch_pending_sensor_readings().await?;
        assert_eq!(pending.len(), 1);
    }
    
    Ok(())
}

#[tokio::test]
async fn sqlite_batch_mark_uploaded() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    let storage = SqliteStorage::new(db_path).await?;

    // create multiple readings
    let reading1 = dummy_reading();
    let reading2 = dummy_reading();
    let reading3 = dummy_reading();
    
    let id1 = reading1.id;
    let id2 = reading2.id;
    let id3 = reading3.id;

    storage.store_sensor_reading(reading1).await?;
    storage.store_sensor_reading(reading2).await?;
    storage.store_sensor_reading(reading3).await?;

    // mark two as uploaded
    storage.mark_sensor_readings_uploaded(&[id1, id2]).await?;

    let pending = storage.fetch_pending_sensor_readings().await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id3);
    
    Ok(())
}

#[tokio::test]
async fn sqlite_empty_ids_handling() -> Result<(), SqliteStorageError> {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();
    
    let storage = SqliteStorage::new(db_path).await?;

    // should not panic with empty slices
    storage.mark_sensor_readings_uploaded(&[]).await?;
    storage.mark_device_statuses_uploaded(&[]).await?;
    
    Ok(())
}
