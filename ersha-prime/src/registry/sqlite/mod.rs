mod device;
mod dispatcher;

#[cfg(test)]
mod tests {
    use ersha_core::Percentage;
    use jiff::Timestamp;
    use ordered_float::NotNan;
    use ulid::Ulid;

    use crate::registry::DeviceRegistry;
    use crate::registry::DispatcherRegistry;
    use crate::registry::filter::DeviceFilter;
    use crate::registry::filter::DeviceSortBy;
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };
    use crate::registry::sqlite::device::SqliteDeviceRegistry;
    use crate::registry::sqlite::dispatcher::SqliteDispatcherRegistry;
    use ersha_core::{
        Device, DeviceId, DeviceKind, DeviceState, Dispatcher, DispatcherId, DispatcherState,
        H3Cell, Sensor, SensorId, SensorKind, SensorMetric,
    };

    use sqlx::SqlitePool;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::SqlitePoolOptions;

    static MIGRATROR: Migrator = sqlx::migrate!("./migrations");

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create pool");

        MIGRATROR
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

    fn dispatcher(
        id: DispatcherId,
        state: DispatcherState,
        provisioned_at: Timestamp,
    ) -> Dispatcher {
        Dispatcher {
            id,
            state,
            location: H3Cell(0x1337deadbeef),
            provisioned_at,
        }
    }

    fn default_options() -> QueryOptions<DispatcherFilter, DispatcherSortBy> {
        QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 100,
            },
        }
    }

    fn mock_device(id: Ulid) -> Device {
        Device {
            id: DeviceId(id),
            kind: DeviceKind::Sensor,
            state: DeviceState::Active,
            location: H3Cell(0x8a2a1072b59ffff),
            manufacturer: Some("TestCorp".to_string().into_boxed_str()),
            provisioned_at: jiff::Timestamp::now(),
            sensors: vec![Sensor {
                id: SensorId(Ulid::new()),
                kind: SensorKind::AirTemp,
                metric: SensorMetric::AirTemp {
                    value: NotNan::new(22.5).unwrap(),
                },
            }]
            .into_boxed_slice(),
        }
    }

    #[tokio::test]
    async fn test_sqlite_register_and_get() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let id = DispatcherId(Ulid::new());
        let dispatcher = dispatcher(id, DispatcherState::Active, Timestamp::now());

        registry.register(dispatcher).await.expect("Save failed");
        let fetched = registry.get(id).await.expect("Query failed");

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_sqlite_list_with_filters() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let id1 = DispatcherId(Ulid::new());
        let d1 = Dispatcher {
            id: id1,
            state: DispatcherState::Active,
            location: H3Cell(1),
            provisioned_at: Timestamp::now(),
        };
        registry.register(d1).await.unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: Some(vec![DispatcherState::Suspended]),
                ..Default::default()
            },
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_empty_filter_fields() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let id = DispatcherId(Ulid::new());
        registry
            .register(dispatcher(id, DispatcherState::Active, Timestamp::now()))
            .await
            .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: None,
                locations: None,
            },
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should ignore empty filters and return all records"
        );
    }

    #[tokio::test]
    async fn test_sqlite_multiple_state_filter() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let ids = [Ulid::new(), Ulid::new(), Ulid::new()];
        registry
            .batch_register(vec![
                dispatcher(
                    DispatcherId(ids[0]),
                    DispatcherState::Active,
                    Timestamp::now(),
                ),
                dispatcher(
                    DispatcherId(ids[1]),
                    DispatcherState::Suspended,
                    Timestamp::now(),
                ),
                dispatcher(
                    DispatcherId(ids[2]),
                    DispatcherState::Suspended,
                    Timestamp::now(),
                ),
            ])
            .await
            .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: Some(vec![DispatcherState::Active]),
                ..Default::default()
            },
            ..default_options()
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results
                .iter()
                .all(|d| d.state != DispatcherState::Suspended)
        );
    }

    #[tokio::test]
    async fn test_sqlite_pagination_offset_logic() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        for i in 0..5 {
            let d = dispatcher(
                DispatcherId(Ulid::new()),
                DispatcherState::Active,
                Timestamp::from_second(i).unwrap(),
            );
            registry.register(d).await.unwrap();
        }

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 2,
                limit: 2,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].provisioned_at,
            Timestamp::from_second(2).unwrap()
        );
        assert_eq!(
            results[1].provisioned_at,
            Timestamp::from_second(3).unwrap()
        );
    }

    #[tokio::test]
    async fn test_sqlite_count_after_suspend() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };
        let id = DispatcherId(Ulid::new());

        registry
            .register(dispatcher(id, DispatcherState::Active, Timestamp::now()))
            .await
            .unwrap();

        let active_filter = DispatcherFilter {
            states: Some(vec![DispatcherState::Active]),
            ..Default::default()
        };

        assert_eq!(
            registry.count(Some(active_filter.clone())).await.unwrap(),
            1
        );

        registry.suspend(id).await.unwrap();

        assert_eq!(registry.count(Some(active_filter)).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_register_and_get_device() {
        let pool = setup_db().await;
        let mut registry = SqliteDeviceRegistry { pool };
        let id = Ulid::new();
        let device = mock_device(id);

        registry
            .register(device.clone())
            .await
            .expect("Should register");

        let fetched = registry
            .get(DeviceId(id))
            .await
            .expect("Should fetch")
            .expect("Device should exist");

        assert_eq!(fetched.id, device.id);
        assert_eq!(fetched.manufacturer, device.manufacturer);
        assert_eq!(fetched.sensors.len(), 1);
        assert!(
            matches!(fetched.sensors[0].metric, SensorMetric::AirTemp { value } if value == 22.5)
        );
    }

    #[tokio::test]
    async fn test_filter_by_manufacturer() {
        let pool = setup_db().await;
        let mut registry = SqliteDeviceRegistry { pool };

        let mut d1 = mock_device(Ulid::new());
        d1.manufacturer = Some("Apple".to_string().into_boxed_str());

        let mut d2 = mock_device(Ulid::new());
        d2.manufacturer = Some("Banana".to_string().into_boxed_str());

        registry.register(d1).await.unwrap();
        registry.register(d2).await.unwrap();

        let filter = DeviceFilter {
            manufacturer_pattern: Some("App".to_string()),
            ..Default::default()
        };

        let options = QueryOptions {
            filter,
            sort_by: DeviceSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manufacturer.as_deref(), Some("Apple"));
    }

    #[tokio::test]
    async fn test_suspend_device() {
        let pool = setup_db().await;
        let mut registry = SqliteDeviceRegistry { pool };

        let id = Ulid::new();
        let device = mock_device(id);

        registry.register(device).await.unwrap();
        registry.suspend(DeviceId(id)).await.unwrap();

        let fetched = registry.get(DeviceId(id)).await.unwrap().unwrap();
        assert_eq!(fetched.state, DeviceState::Suspended);
    }

    #[tokio::test]
    async fn test_add_sensor_individually() {
        let pool = setup_db().await;
        let mut registry = SqliteDeviceRegistry { pool };

        let d_id = Ulid::new();
        let mut device = mock_device(d_id);
        device.sensors = vec![].into_boxed_slice(); // Start with none

        registry.register(device).await.unwrap();

        let new_sensor = Sensor {
            id: SensorId(Ulid::new()),
            kind: SensorKind::Humidity,
            metric: SensorMetric::Humidity {
                value: Percentage(45),
            },
        };

        registry
            .add_sensor(DeviceId(d_id), new_sensor)
            .await
            .unwrap();

        let fetched = registry.get(DeviceId(d_id)).await.unwrap().unwrap();
        assert_eq!(fetched.sensors.len(), 1);
        assert!(matches!(fetched.sensors[0].kind, SensorKind::Humidity));
    }
}
