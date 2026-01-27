pub mod models;
pub mod devices;
pub mod dispatchers;
pub mod error;

use axum::{
    Router,
    routing::{get, post, put, delete},
};

use crate::AppState;

pub fn router<DR, DisR>() -> Router<AppState<DR, DisR>>
where
    DR: crate::registry::DeviceRegistry + Clone + Send + Sync + 'static,
    DisR: crate::registry::DispatcherRegistry + Clone + Send + Sync + 'static,
{
    Router::new()
        // Device routes 
        .route("/devices", 
            get(devices::list_devices)
            .post(devices::create_device)
        )
        .route("/devices/{id}",  
            get(devices::get_device)
            .put(devices::update_device)
            .delete(devices::delete_device)
        )
        .route("/devices/{id}/sensors",  
            post(devices::add_sensor)
        )
        // Dispatcher routes 
        .route("/dispatchers", 
            get(dispatchers::list_dispatchers)
            .post(dispatchers::create_dispatcher)
        )
        .route("/dispatchers/{id}",  
            get(dispatchers::get_dispatcher)
            .put(dispatchers::update_dispatcher)
            .delete(dispatchers::delete_dispatcher)
        )
}
