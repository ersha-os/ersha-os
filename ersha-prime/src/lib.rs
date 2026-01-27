pub mod api;
pub mod config;
pub mod registry;

// AppState must be defined in lib.rs to be visible to all modules
#[derive(Clone)]
pub struct AppState<DR, DisR> {
    pub device_registry: DR,
    pub dispatcher_registry: DisR,
}
