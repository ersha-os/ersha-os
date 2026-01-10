#![allow(dead_code)]
pub mod filter;
pub mod memory;
pub mod sqlite;

use ersha_core::{Device, DeviceId, Dispatcher, DispatcherId, Sensor};
use filter::{DeviceFilter, DeviceSortBy, DispatcherFilter, DispatcherSortBy, QueryOptions};

trait DeviceRegistry {
    type Error;

    async fn register(&mut self, device: Device) -> Result<(), Self::Error>;
    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error>;
    async fn update(&mut self, id: DeviceId, new: Device) -> Result<(), Self::Error>;
    async fn suspend(&mut self, id: DeviceId) -> Result<(), Self::Error>;

    async fn add_sensor(&mut self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error>;
    async fn add_sensors(
        &mut self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor>,
    ) -> Result<(), Self::Error>;
    async fn batch_register(&mut self, devices: Vec<Device>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error>;
}

trait DispatcherRegistry {
    type Error;

    async fn register(&mut self, dispatcher: Dispatcher) -> Result<(), Self::Error>;
    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error>;
    async fn update(&mut self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error>;
    async fn suspend(&mut self, id: DispatcherId) -> Result<(), Self::Error>;

    async fn batch_register(&mut self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error>;
}
