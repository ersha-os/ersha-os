pub mod mock;
pub mod tcp;

use async_trait::async_trait;
use ersha_core::{DeviceStatus, SensorReading};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Data received from edge devices.
#[derive(Debug, Clone)]
pub enum EdgeData {
    /// A sensor reading from a device.
    Reading(SensorReading),
    /// A device status report.
    Status(DeviceStatus),
}

/// Trait for receiving data from edge devices.
///
/// Implementations of this trait spawn background tasks that send data
/// to an mpsc channel. The receiver is returned from the `start` method.
#[async_trait]
pub trait EdgeReceiver: Send + Sync + 'static {
    /// Error type for this edge receiver implementation.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Start receiving data from edge devices.
    ///
    /// Returns a channel receiver that will receive data from edge devices.
    /// The background tasks will run until the cancellation token is cancelled.
    async fn start(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<EdgeData>, Self::Error>;
}
