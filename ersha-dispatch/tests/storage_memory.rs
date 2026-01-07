use ersha_dispatch::storage::memory::MemoryStorage;
use ersha_dispatch::storage::Storage;
use ersha_core::*;
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

#[tokio::test]
async fn sensor_reading_lifecycle() {
    let storage = MemoryStorage::default();

    let reading = dummy_reading();
    let reading_id = reading.id;

    storage.store_sensor_reading(reading).await.unwrap();

    let pending = storage.fetch_pending_sensor_readings().await.unwrap();
    assert_eq!(pending.len(), 1);

    storage
        .mark_sensor_readings_uploaded(&[reading_id])
        .await
        .unwrap();

    let pending = storage.fetch_pending_sensor_readings().await.unwrap();
    assert_eq!(pending.len(), 0);
}

