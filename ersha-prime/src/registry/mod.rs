pub mod filter;
pub mod memory;
pub mod sqlite;

use async_trait::async_trait;
use ersha_core::{
    Device, DeviceId, DeviceStatus, Dispatcher, DispatcherId, ReadingId, Sensor, SensorReading,
    StatusId,
};
use filter::{
    DeviceFilter, DeviceSortBy, DeviceStatusFilter, DeviceStatusSortBy, DispatcherFilter,
    DispatcherSortBy, QueryOptions, ReadingFilter, ReadingSortBy,
};

#[async_trait]
pub trait DeviceRegistry: Clone + Send + Sync + 'static {
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
pub trait DispatcherRegistry: Clone + Send + Sync + 'static {
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

#[async_trait]
pub trait ReadingRegistry: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn store(&self, reading: SensorReading) -> Result<(), Self::Error>;
    async fn get(&self, id: ReadingId) -> Result<Option<SensorReading>, Self::Error>;
    async fn batch_store(&self, readings: Vec<SensorReading>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<ReadingFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<ReadingFilter, ReadingSortBy>,
    ) -> Result<Vec<SensorReading>, Self::Error>;
}

#[async_trait]
pub trait DeviceStatusRegistry: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn store(&self, status: DeviceStatus) -> Result<(), Self::Error>;
    async fn get(&self, id: StatusId) -> Result<Option<DeviceStatus>, Self::Error>;
    async fn get_latest(&self, device_id: DeviceId) -> Result<Option<DeviceStatus>, Self::Error>;
    async fn batch_store(&self, statuses: Vec<DeviceStatus>) -> Result<(), Self::Error>;
    async fn count(&self, filter: Option<DeviceStatusFilter>) -> Result<usize, Self::Error>;
    async fn list(
        &self,
        options: QueryOptions<DeviceStatusFilter, DeviceStatusSortBy>,
    ) -> Result<Vec<DeviceStatus>, Self::Error>;
}
