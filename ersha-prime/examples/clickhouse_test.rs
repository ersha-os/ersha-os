use color_eyre::eyre::Result;
use ersha_core::{
    Device, DeviceId, DeviceKind, DeviceState, DeviceStatus, Dispatcher, DispatcherId,
    DispatcherState, H3Cell, Percentage, ReadingId, Sensor, SensorId, SensorKind, SensorMetric,
    SensorReading, StatusId,
};
use ersha_prime::registry::{
    DeviceRegistry, DeviceStatusRegistry, DispatcherRegistry, ReadingRegistry,
    clickhouse::{
        ClickHouseDeviceRegistry, ClickHouseDeviceStatusRegistry, ClickHouseDispatcherRegistry,
        ClickHouseReadingRegistry,
    },
};
use jiff::Timestamp;
use ordered_float::NotNan;
use ulid::Ulid;

const URL: &str = "http://localhost:8123";
const DATABASE: &str = "ersha_test";

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    println!("=== ClickHouse Integration Tests ===\n");

    // First, create the database
    println!("Creating test database...");
    create_database().await?;
    println!("  ✓ Database created\n");

    // Test Dispatcher Registry
    println!("Testing DispatcherRegistry...");
    test_dispatcher_registry().await?;
    println!("  ✓ DispatcherRegistry tests passed\n");

    // Test Device Registry
    println!("Testing DeviceRegistry...");
    test_device_registry().await?;
    println!("  ✓ DeviceRegistry tests passed\n");

    // Test Reading Registry
    println!("Testing ReadingRegistry...");
    test_reading_registry().await?;
    println!("  ✓ ReadingRegistry tests passed\n");

    // Test Device Status Registry
    println!("Testing DeviceStatusRegistry...");
    test_device_status_registry().await?;
    println!("  ✓ DeviceStatusRegistry tests passed\n");

    println!("=== All tests passed! ===");
    Ok(())
}

async fn create_database() -> Result<()> {
    let client = clickhouse::Client::default().with_url(URL);
    client
        .query("CREATE DATABASE IF NOT EXISTS ersha_test")
        .execute()
        .await?;
    Ok(())
}

async fn test_dispatcher_registry() -> Result<()> {
    let registry = ClickHouseDispatcherRegistry::new(URL, DATABASE).await?;

    let id = DispatcherId(Ulid::new());
    let dispatcher = Dispatcher {
        id,
        state: DispatcherState::Active,
        location: H3Cell(0x8a2a1072b59ffff),
        provisioned_at: Timestamp::now(),
    };

    // Register
    registry.register(dispatcher.clone()).await?;
    print!("  - register: ok, ");

    // Get
    let fetched = registry.get(id).await?.expect("dispatcher should exist");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.state, DispatcherState::Active);
    print!("get: ok, ");

    // Count
    let count = registry.count(None).await?;
    assert!(count >= 1);
    print!("count: ok, ");

    // Suspend
    registry.suspend(id).await?;
    let suspended = registry.get(id).await?.expect("dispatcher should exist");
    assert_eq!(suspended.state, DispatcherState::Suspended);
    println!("suspend: ok");

    Ok(())
}

async fn test_device_registry() -> Result<()> {
    let registry = ClickHouseDeviceRegistry::new(URL, DATABASE).await?;

    let id = DeviceId(Ulid::new());
    let device = Device {
        id,
        kind: DeviceKind::Sensor,
        state: DeviceState::Active,
        location: H3Cell(0x8a2a1072b59ffff),
        manufacturer: Some("TestCorp".into()),
        provisioned_at: Timestamp::now(),
        sensors: vec![Sensor {
            id: SensorId(Ulid::new()),
            kind: SensorKind::AirTemp,
            metric: SensorMetric::AirTemp {
                value: NotNan::new(22.5).unwrap(),
            },
        }]
        .into_boxed_slice(),
    };

    // Register
    registry.register(device.clone()).await?;
    print!("  - register: ok, ");

    // Get
    let fetched = registry.get(id).await?.expect("device should exist");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.sensors.len(), 1);
    print!("get: ok, ");

    // Count
    let count = registry.count(None).await?;
    assert!(count >= 1);
    print!("count: ok, ");

    // Suspend
    registry.suspend(id).await?;
    let suspended = registry.get(id).await?.expect("device should exist");
    assert_eq!(suspended.state, DeviceState::Suspended);
    println!("suspend: ok");

    Ok(())
}

async fn test_reading_registry() -> Result<()> {
    let registry = ClickHouseReadingRegistry::new(URL, DATABASE).await?;

    let id = ReadingId(Ulid::new());
    let reading = SensorReading {
        id,
        device_id: DeviceId(Ulid::new()),
        dispatcher_id: DispatcherId(Ulid::new()),
        sensor_id: SensorId(Ulid::new()),
        metric: SensorMetric::AirTemp {
            value: NotNan::new(25.0).unwrap(),
        },
        location: H3Cell(0x8a2a1072b59ffff),
        confidence: Percentage(95),
        timestamp: Timestamp::now(),
    };

    // Store
    registry.store(reading.clone()).await?;
    print!("  - store: ok, ");

    // Get
    let fetched = registry.get(id).await?.expect("reading should exist");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.confidence.0, 95);
    print!("get: ok, ");

    // Batch store
    let readings = vec![
        SensorReading {
            id: ReadingId(Ulid::new()),
            device_id: DeviceId(Ulid::new()),
            dispatcher_id: DispatcherId(Ulid::new()),
            sensor_id: SensorId(Ulid::new()),
            metric: SensorMetric::Humidity {
                value: Percentage(60),
            },
            location: H3Cell(0x8a2a1072b59ffff),
            confidence: Percentage(90),
            timestamp: Timestamp::now(),
        },
        SensorReading {
            id: ReadingId(Ulid::new()),
            device_id: DeviceId(Ulid::new()),
            dispatcher_id: DispatcherId(Ulid::new()),
            sensor_id: SensorId(Ulid::new()),
            metric: SensorMetric::SoilMoisture {
                value: Percentage(45),
            },
            location: H3Cell(0x8a2a1072b59ffff),
            confidence: Percentage(85),
            timestamp: Timestamp::now(),
        },
    ];
    registry.batch_store(readings).await?;
    print!("batch_store: ok, ");

    // Count
    let count = registry.count(None).await?;
    assert!(count >= 3);
    println!("count: ok");

    Ok(())
}

async fn test_device_status_registry() -> Result<()> {
    let registry = ClickHouseDeviceStatusRegistry::new(URL, DATABASE).await?;

    let device_id = DeviceId(Ulid::new());
    let id = StatusId(Ulid::new());
    let status = DeviceStatus {
        id,
        device_id,
        dispatcher_id: DispatcherId(Ulid::new()),
        battery_percent: Percentage(85),
        uptime_seconds: 3600,
        signal_rssi: -50,
        errors: vec![].into_boxed_slice(),
        timestamp: Timestamp::now(),
        sensor_statuses: vec![].into_boxed_slice(),
    };

    // Store
    registry.store(status.clone()).await?;
    print!("  - store: ok, ");

    // Get
    let fetched = registry.get(id).await?.expect("status should exist");
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.battery_percent.0, 85);
    print!("get: ok, ");

    // Get latest
    let latest = registry
        .get_latest(device_id)
        .await?
        .expect("status should exist");
    assert_eq!(latest.device_id, device_id);
    print!("get_latest: ok, ");

    // Count
    let count = registry.count(None).await?;
    assert!(count >= 1);
    println!("count: ok");

    Ok(())
}
