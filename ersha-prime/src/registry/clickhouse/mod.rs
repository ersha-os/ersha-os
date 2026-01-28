mod device;
mod device_status;
mod dispatcher;
mod reading;

pub use device::ClickHouseDeviceRegistry;
pub use device_status::ClickHouseDeviceStatusRegistry;
pub use dispatcher::ClickHouseDispatcherRegistry;
pub use reading::ClickHouseReadingRegistry;

use clickhouse::Client;

/// Shared error type for all ClickHouse registry implementations.
#[derive(Debug, thiserror::Error)]
pub enum ClickHouseError {
    #[error("clickhouse error: {0}")]
    Client(#[from] clickhouse::error::Error),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("invalid metric type: {0}")]
    InvalidMetricType(i32),
    #[error("invalid device state: {0}")]
    InvalidDeviceState(i32),
    #[error("invalid device kind: {0}")]
    InvalidDeviceKind(i32),
    #[error("invalid sensor kind: {0}")]
    InvalidSensorKind(i32),
    #[error("invalid dispatcher state: {0}")]
    InvalidDispatcherState(i32),
    #[error("invalid error code: {0}")]
    InvalidErrorCode(i32),
    #[error("invalid sensor state: {0}")]
    InvalidSensorState(i32),
    #[error("entity not found")]
    NotFound,
}

/// Creates a ClickHouse client configured for the given URL and database.
pub fn create_client(url: &str, database: &str) -> Client {
    Client::default().with_url(url).with_database(database)
}
