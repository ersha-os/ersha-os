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
