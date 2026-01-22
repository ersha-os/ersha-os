#![allow(dead_code)]
pub mod filter;
pub mod memory;
pub mod sqlite;

use async_trait::async_trait;
use ersha_core::{Device, DeviceId, Dispatcher, DispatcherId, Sensor};
use filter::{DeviceFilter, DeviceSortBy, DispatcherFilter, DispatcherSortBy, QueryOptions};

#[async_trait]
trait DeviceRegistry: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn register(&self, device: Device) -> Result<(), Self::Error>;
    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error>;
    async fn update(&self, id: DeviceId, new: Device) -> Result<(), Self::Error>;
    async fn suspend(&self, id: DeviceId) -> Result<(), Self::Error>;

    async fn add_sensor(&self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error>;
    async fn add_sensors(
        &self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor> + Send,
    ) -> Result<(), Self::Error>;
    async fn batch_register(&self, devices: Vec<Device>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error>;
}

#[async_trait]
trait DispatcherRegistry: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn register(&self, dispatcher: Dispatcher) -> Result<(), Self::Error>;
    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error>;
    async fn update(&self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error>;
    async fn suspend(&self, id: DispatcherId) -> Result<(), Self::Error>;

    async fn batch_register(&self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error>;
}
