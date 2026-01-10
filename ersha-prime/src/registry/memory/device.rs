use std::collections::HashMap;

use ersha_core::{Device, DeviceId, DeviceState};

use crate::registry::{
    DeviceRegistry,
    filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder},
};

use super::InMemoryError;

pub struct InMemoryDeviceRegistry {
    pub devices: HashMap<DeviceId, Device>,
}

impl DeviceRegistry for InMemoryDeviceRegistry {
    type Error = InMemoryError;

    async fn register(&mut self, device: Device) -> Result<(), Self::Error> {
        let _ = self.devices.insert(device.id, device);

        Ok(())
    }

    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error> {
        Ok(self.devices.get(&id).cloned())
    }

    async fn update(&mut self, id: DeviceId, new: Device) -> Result<(), Self::Error> {
        let _old = self.devices.insert(id, new);
        Ok(())
    }

    async fn suspend(&mut self, id: DeviceId) -> Result<(), Self::Error> {
        let device = self.get(id).await?.ok_or(InMemoryError::NotFound)?;

        let _ = self
            .update(
                id,
                Device {
                    state: DeviceState::Suspended,
                    ..device
                },
            )
            .await?;

        Ok(())
    }

    async fn batch_register(&mut self, devices: Vec<Device>) -> Result<(), Self::Error> {
        for device in devices {
            self.register(device).await?;
        }

        Ok(())
    }

    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error> {
        if let Some(filter) = filter {
            let filtered = filter_devices(&self.devices, &filter);

            return Ok(filtered.count());
        }

        Ok(self.devices.len())
    }

    async fn list(
        &self,
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error> {
        let filtered: Vec<&Device> = filter_devices(&self.devices, &options.filter).collect();
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
                let id = DeviceId(inner_ulid.clone());
                return devices
                    .into_iter()
                    .skip_while(|device| device.id != id)
                    .skip(1)
                    .take(*limit)
                    .cloned()
                    .collect();
            }

            return vec![];
        }
    }
}

fn filter_devices<'a>(
    devices: &'a HashMap<DeviceId, Device>,
    filter: &DeviceFilter,
) -> impl Iterator<Item = &'a Device> {
    devices.values().filter_map(|device| {
        if let Some(locations) = &filter.locations {
            if !locations.contains(&device.location) {
                return None;
            }
        }

        if let Some(states) = &filter.states {
            if !states.contains(&device.state) {
                return None;
            }
        }

        if let Some(kinds) = &filter.kinds {
            if !kinds.contains(&device.kind) {
                return None;
            }
        }

        if let Some(pattern) = &filter.manufacturer_pattern {
            match &device.manufacturer {
                Some(manufacturer) => {
                    if !manufacturer
                        .to_lowercase()
                        .contains(&pattern.to_lowercase())
                    {
                        return None;
                    }
                }
                None => return None,
            };
        }

        if let Some(sensor_range) = &filter.sensor_count {
            if !sensor_range.contains(&device.sensors.len()) {
                return None;
            }
        }

        match (&filter.provisioned_after, &filter.provisioned_before) {
            (None, None) => (),
            (None, Some(before)) => {
                if &device.provisioned_at > before {
                    return None;
                }
            }
            (Some(after), None) => {
                if &device.provisioned_at < after {
                    return None;
                }
            }
            (Some(after), Some(before)) => {
                if &device.provisioned_at < after || &device.provisioned_at > before {
                    return None;
                }
            }
        }

        Some(device)
    })
}
