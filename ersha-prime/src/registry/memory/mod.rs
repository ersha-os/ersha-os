mod device;
mod dispatcher;

#[derive(Debug, thiserror::Error)]
pub enum InMemoryError {
    #[error("not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use ersha_core::Sensor;
    use ersha_core::SensorId;
    use ersha_core::SensorKind;
    use ersha_core::SensorMetric;
    use jiff::Timestamp;
    use ordered_float::NotNan;
    use std::collections::HashMap;
    use ulid::Ulid;

    use crate::registry::DeviceRegistry;
    use crate::registry::DispatcherRegistry;
    use crate::registry::filter::DeviceFilter;
    use crate::registry::filter::DeviceSortBy;
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{
        Device, DeviceId, DeviceKind, DeviceState, Dispatcher, DispatcherId, DispatcherState,
        H3Cell,
    };

    use super::device::InMemoryDeviceRegistry;
    use super::dispatcher::InMemoryDispatcherRegistry;

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

    fn mock_device(id: Ulid, manufacturer: &str) -> Device {
        Device {
            id: DeviceId(id),
            kind: DeviceKind::Sensor,
            state: DeviceState::Active,
            location: H3Cell(0x8a2a1072b59ffff),
            manufacturer: Some(manufacturer.to_string().into_boxed_str()),
            provisioned_at: Timestamp::now(),
            sensors: vec![].into_boxed_slice(),
        }
    }

    fn dispatcher_registry() -> InMemoryDispatcherRegistry {
        InMemoryDispatcherRegistry {
            dispatchers: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    fn device_registry() -> InMemoryDeviceRegistry {
        InMemoryDeviceRegistry {
            devices: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let reg = dispatcher_registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d.clone()).await.unwrap();
        let fetched = reg.get(id).await.unwrap().expect("Dispatcher should exist");

        assert_eq!(fetched.id, id);
        assert_eq!(fetched.state, DispatcherState::Active);
    }

    #[tokio::test]
    async fn test_suspend_logic() {
        let reg = dispatcher_registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d).await.unwrap();
        reg.suspend(id).await.unwrap();

        let updated = reg.get(id).await.unwrap().unwrap();
        assert_eq!(updated.state, DispatcherState::Suspended);
    }

    #[tokio::test]
    async fn test_count_with_filter() {
        let reg = dispatcher_registry();
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());

        reg.batch_register(vec![
            dispatcher(id1, DispatcherState::Active, Timestamp::now()),
            dispatcher(id2, DispatcherState::Suspended, Timestamp::now()),
        ])
        .await
        .unwrap();

        assert_eq!(reg.count(None).await.unwrap(), 2);

        let filter = DispatcherFilter {
            states: Some(vec![DispatcherState::Active]),
            ..Default::default()
        };
        assert_eq!(reg.count(Some(filter)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_sorting_and_pagination() {
        let reg = dispatcher_registry();

        // Create 3 dispatchers with distinct timestamps
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());
        let id3 = DispatcherId(Ulid::new());

        reg.batch_register(vec![
            dispatcher(
                id1,
                DispatcherState::Active,
                Timestamp::from_second(100).unwrap(),
            ),
            dispatcher(
                id2,
                DispatcherState::Active,
                Timestamp::from_second(300).unwrap(),
            ),
            dispatcher(
                id3,
                DispatcherState::Active,
                Timestamp::from_second(200).unwrap(),
            ),
        ])
        .await
        .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Desc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 2,
            },
        };

        let results = reg.list(options).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, id2);
        assert_eq!(results[1].id, id3);
    }

    #[tokio::test]
    async fn test_cursor_pagination() {
        let reg = dispatcher_registry();
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());

        // Important for cursor: In-memory hashmap order is random,
        // but our sort_dispatchers makes it deterministic
        reg.batch_register(vec![
            dispatcher(
                id1,
                DispatcherState::Active,
                Timestamp::from_second(10).unwrap(),
            ),
            dispatcher(
                id2,
                DispatcherState::Active,
                Timestamp::from_second(20).unwrap(),
            ),
        ])
        .await
        .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Cursor {
                after: Some(id1.0),
                limit: 1,
            },
        };

        let results = reg.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id2);
    }

    #[tokio::test]
    async fn test_register_and_get_device() {
        let registry = device_registry();
        let id = Ulid::new();
        let device = mock_device(id, "Apple");

        registry.register(device.clone()).await.unwrap();

        let fetched = registry.get(DeviceId(id)).await.unwrap().unwrap();
        assert_eq!(fetched.manufacturer.as_deref(), Some("Apple"));
    }

    #[tokio::test]
    async fn test_add_sensor() {
        let registry = device_registry();

        let d_id = Ulid::new();
        registry
            .register(mock_device(d_id, "SensorCo"))
            .await
            .unwrap();

        let sensor = Sensor {
            id: SensorId(Ulid::new()),
            kind: SensorKind::AirTemp,
            metric: SensorMetric::AirTemp {
                value: NotNan::new(25.0).unwrap(),
            },
        };

        registry.add_sensor(DeviceId(d_id), sensor).await.unwrap();

        let fetched = registry.get(DeviceId(d_id)).await.unwrap().unwrap();
        assert_eq!(fetched.sensors.len(), 1);
        assert!(
            matches!(fetched.sensors[0].metric, SensorMetric::AirTemp { value } if value == 25.0)
        );
    }

    #[tokio::test]
    async fn test_in_memory_filtering_and_sorting() {
        let registry = device_registry();

        let d1 = mock_device(Ulid::new(), "Alpha");

        let mut d2 = mock_device(Ulid::new(), "Beta");
        d2.sensors = vec![Sensor {
            id: SensorId(Ulid::new()),
            kind: SensorKind::Rainfall,
            metric: SensorMetric::Rainfall {
                value: NotNan::new(1.0).unwrap(),
            },
        }]
        .into_boxed_slice();

        let d3 = mock_device(Ulid::new(), "Gamma");

        registry.batch_register(vec![d1, d2, d3]).await.unwrap();

        let filter = DeviceFilter {
            sensor_count: Some(1..=1),
            ..Default::default()
        };

        let options = QueryOptions {
            filter,
            sort_by: DeviceSortBy::Manufacturer,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manufacturer.as_deref(), Some("Beta"));
    }

    #[tokio::test]
    async fn test_in_memory_pagination_offset() {
        let registry = device_registry();

        for i in 0..5 {
            registry
                .register(mock_device(Ulid::new(), &format!("Dev-{}", i)))
                .await
                .unwrap();
        }

        let options = QueryOptions {
            filter: DeviceFilter::default(),
            sort_by: DeviceSortBy::Manufacturer, // Sort alphabetically
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 1,
                limit: 2,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].manufacturer.as_deref(), Some("Dev-1"));
        assert_eq!(results[1].manufacturer.as_deref(), Some("Dev-2"));
    }

    #[tokio::test]
    async fn test_in_memory_cursor_pagination() {
        let registry = device_registry();

        let id1 = Ulid::new();
        let id2 = Ulid::new();
        let id3 = Ulid::new();

        registry.register(mock_device(id1, "A")).await.unwrap();
        registry.register(mock_device(id2, "B")).await.unwrap();
        registry.register(mock_device(id3, "C")).await.unwrap();

        let options = QueryOptions {
            filter: DeviceFilter::default(),
            sort_by: DeviceSortBy::Manufacturer,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Cursor {
                after: Some(id1),
                limit: 1,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, DeviceId(id2));
    }
}
