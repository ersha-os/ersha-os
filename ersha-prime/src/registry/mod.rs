#![allow(dead_code)]
pub mod filter;

use ersha_core::{Device, DeviceId, Dispatcher, DispatcherId};
use filter::{DeviceFilter, DispatcherFilter, QueryOptions};

trait DeviceRegistry {
    type Error;

    async fn register(&mut self, device: Device) -> Result<(), Self::Error>;
    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error>;
    async fn update(&mut self, id: DeviceId, new: Device) -> Result<(), Self::Error>;
    async fn suspend(&mut self, id: DeviceId) -> Result<(), Self::Error>;

    async fn batch_register(&mut self, devices: Vec<Device>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error>;
    async fn list(&self, options: QueryOptions<DeviceFilter>) -> Result<Vec<Device>, Self::Error>;
}

trait DispatcherRegistry {
    type Error;

    async fn register(&mut self, dispatcher: Dispatcher) -> Result<(), Self::Error>;
    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error>;
    async fn update(&mut self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error>;
    async fn suspend(&mut self, id: DispatcherId) -> Result<(), Self::Error>;

    async fn batch_register(&mut self, devices: Vec<Dispatcher>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter>,
    ) -> Result<Vec<Dispatcher>, Self::Error>;
}
