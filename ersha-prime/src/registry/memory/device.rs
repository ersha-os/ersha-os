use std::collections::HashMap;

use async_trait::async_trait;
use ersha_core::{Device, DeviceId, DeviceState, Sensor};
use tokio::sync::RwLock;

use crate::registry::{
    DeviceRegistry,
    filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder},
};

use super::InMemoryError;

pub struct InMemoryDeviceRegistry {
    pub devices: RwLock<HashMap<DeviceId, Device>>,
}

#[async_trait]
impl DeviceRegistry for InMemoryDeviceRegistry {
    type Error = InMemoryError;

    async fn register(&self, device: Device) -> Result<(), Self::Error> {
        let mut devices = self.devices.write().await;
        let _ = devices.insert(device.id, device);

        Ok(())
    }

    async fn add_sensor(&self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error> {
        let mut devices = self.devices.write().await;
        let mut device = devices.get(&id).cloned().ok_or(InMemoryError::NotFound)?;

        device.sensors = device
            .sensors
            .into_iter()
            .chain(std::iter::once(sensor))
            .collect::<Box<[Sensor]>>();

        let new = Device { ..device };
        devices.insert(id, new);
        Ok(())
    }

    async fn add_sensors(
        &self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor> + Send,
    ) -> Result<(), Self::Error> {
        let mut devices = self.devices.write().await;
        let mut device = devices.get(&id).cloned().ok_or(InMemoryError::NotFound)?;

        device.sensors = device.sensors.into_iter().chain(sensors).collect();

        devices.insert(id, device);
        Ok(())
    }

    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error> {
        let devices = self.devices.read().await;
        Ok(devices.get(&id).cloned())
    }

    async fn update(&self, id: DeviceId, new: Device) -> Result<(), Self::Error> {
        let mut devices = self.devices.write().await;
        let _old = devices.insert(id, new);
        Ok(())
    }

    async fn suspend(&self, id: DeviceId) -> Result<(), Self::Error> {
        let device = self.get(id).await?.ok_or(InMemoryError::NotFound)?;

        self.update(
            id,
            Device {
                state: DeviceState::Suspended,
                ..device
            },
        )
        .await?;

        Ok(())
    }

    async fn batch_register(&self, devices: Vec<Device>) -> Result<(), Self::Error> {
        for device in devices {
            self.register(device).await?;
        }

        Ok(())
    }

    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error> {
        let devices = self.devices.read().await;
        if let Some(filter) = filter {
            let filtered = filter_devices(&devices, &filter);

            return Ok(filtered.count());
        }

        Ok(devices.len())
    }

    async fn list(
        &self,
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error> {
        let devices = self.devices.read().await;
        let filtered: Vec<&Device> = filter_devices(&devices, &options.filter).collect();
        let sorted = sort_devices(filtered, &options.sort_by, &options.sort_order);
        let paginated = paginate_devices(sorted, &options.pagination);

        Ok(paginated)
    }
}

fn sort_devices<'a>(
    mut devices: Vec<&'a Device>,
    sort_by: &DeviceSortBy,
    sort_order: &SortOrder,
) -> Vec<&'a Device> {
    devices.sort_by(|a, b| {
        let ord = match sort_by {
            DeviceSortBy::State => (a.state.clone() as i32).cmp(&(b.state.clone() as i32)),
            DeviceSortBy::Manufacturer => a.manufacturer.cmp(&b.manufacturer),
            DeviceSortBy::ProvisionAt => a.provisioned_at.cmp(&b.provisioned_at),
            DeviceSortBy::SensorCount => a.sensors.len().cmp(&b.sensors.len()),
        };

        match sort_order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        }
    });

    devices
}

fn paginate_devices(devices: Vec<&Device>, pagination: &Pagination) -> Vec<Device> {
    match pagination {
        Pagination::Offset { offset, limit } => devices
            .into_iter()
            .skip(*offset)
            .take(*limit)
            .cloned()
            .collect(),
        Pagination::Cursor { after, limit } => {
            if let Some(inner_ulid) = after {
                let id = DeviceId(*inner_ulid);
                return devices
                    .into_iter()
                    .skip_while(|device| device.id != id)
                    .skip(1)
                    .take(*limit)
                    .cloned()
                    .collect();
            }

            vec![]
        }
    }
}

fn filter_devices<'a>(
    devices: &'a HashMap<DeviceId, Device>,
    filter: &DeviceFilter,
) -> impl Iterator<Item = &'a Device> {
    devices.values().filter(|device| {
        if let Some(locations) = &filter.locations
            && !locations.contains(&device.location)
        {
            return false;
        }

        if let Some(states) = &filter.states
            && !states.contains(&device.state)
        {
            return false;
        }

        if let Some(kinds) = &filter.kinds
            && !kinds.contains(&device.kind)
        {
            return false;
        }

        if let Some(pattern) = &filter.manufacturer_pattern {
            match &device.manufacturer {
                Some(manufacturer) => {
                    if !manufacturer
                        .to_lowercase()
                        .contains(&pattern.to_lowercase())
                    {
                        return false;
                    }
                }
                None => return false,
            };
        }

        if let Some(sensor_range) = &filter.sensor_count
            && !sensor_range.contains(&device.sensors.len())
        {
            return false;
        }

        match (&filter.provisioned_after, &filter.provisioned_before) {
            (None, None) => (),
            (None, Some(before)) => {
                if &device.provisioned_at > before {
                    return false;
                }
            }
            (Some(after), None) => {
                if &device.provisioned_at < after {
                    return false;
                }
            }
            (Some(after), Some(before)) => {
                if &device.provisioned_at < after || &device.provisioned_at > before {
                    return false;
                }
            }
        }

        true
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use ulid::Ulid;

    use crate::registry::DeviceRegistry;
    use crate::registry::filter::{
        DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{
        Device, DeviceId, DeviceKind, DeviceState, H3Cell, Sensor, SensorId, SensorKind,
        SensorMetric,
    };
    use ordered_float::NotNan;

    use super::InMemoryDeviceRegistry;

    fn mock_device(id: Ulid, manufacturer: &str) -> Device {
        Device {
            id: DeviceId(id),
            kind: DeviceKind::Sensor,
            state: DeviceState::Active,
            location: H3Cell(0x8a2a1072b59ffff),
            manufacturer: Some(manufacturer.to_string().into_boxed_str()),
            provisioned_at: jiff::Timestamp::now(),
            sensors: vec![].into_boxed_slice(),
        }
    }

    fn device_registry() -> InMemoryDeviceRegistry {
        InMemoryDeviceRegistry {
            devices: tokio::sync::RwLock::new(HashMap::new()),
        }
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
