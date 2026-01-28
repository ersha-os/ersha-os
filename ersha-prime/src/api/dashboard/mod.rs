mod error;
mod handlers;
mod models;

pub use error::DashboardError;
pub use handlers::dashboard_router;
pub use models::{DeviceForm, DispatcherForm, SensorForm};

// Re-export for convenience
pub use handlers::dashboard_index;

// Re-export ApiState for dashboard compatibility
pub type DashboardState<D, Dev> = super::ApiState<D, Dev>;
