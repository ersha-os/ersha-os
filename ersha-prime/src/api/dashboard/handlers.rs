use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post, put, delete},
    Router,
};
use serde::Deserialize;
use ulid::Ulid;

use ersha_core::{
    Device, DeviceId, DeviceKind, DeviceState, Dispatcher, DispatcherId, 
    DispatcherState, H3Cell, Sensor, SensorId, SensorKind, SensorMetric,
};

use crate::registry::{DeviceRegistry, DispatcherRegistry};

use super::{
    error::DashboardError,
    models::{
        DeviceForm, DispatcherForm, SensorForm, 
        ApiResponse, PaginatedResponse, PaginationQuery,
    },
    DashboardState,
};

/// Helper functions
fn parse_ulid(id: &str) -> Result<Ulid, DashboardError> {
    id.parse::<Ulid>()
        .map_err(|_| DashboardError::InvalidUlid(format!("Invalid ID format: {}", id)))
}

fn create_sensors_from_forms(
    sensor_forms: Vec<SensorForm>,
) -> Result<Vec<Sensor>, DashboardError> {
    let mut sensors = Vec::with_capacity(sensor_forms.len());
    
    for sensor_form in sensor_forms {
        let kind = match sensor_form.kind.as_str() {
            "soil_moisture" => SensorKind::SoilMoisture,
            "soil_temp" => SensorKind::SoilTemp,
            "air_temp" => SensorKind::AirTemp,
            "humidity" => SensorKind::Humidity,
            "rainfall" => SensorKind::Rainfall,
            _ => return Err(DashboardError::InvalidSensorKind(sensor_form.kind)),
        };
        
        let metric = match kind {
            SensorKind::SoilMoisture => SensorMetric::SoilMoisture { 
                value: ersha_core::Percentage(0) 
            },
            SensorKind::SoilTemp => SensorMetric::SoilTemp { 
                value: ordered_float::NotNan::new(0.0).unwrap() 
            },
            SensorKind::AirTemp => SensorMetric::AirTemp { 
                value: ordered_float::NotNan::new(0.0).unwrap() 
            },
            SensorKind::Humidity => SensorMetric::Humidity { 
                value: ersha_core::Percentage(0) 
            },
            SensorKind::Rainfall => SensorMetric::Rainfall { 
                value: ordered_float::NotNan::new(0.0).unwrap() 
            },
        };

        sensors.push(Sensor {
            id: SensorId(Ulid::new()),
            kind,
            metric,
        });
    }
    
    Ok(sensors)
}

/// GET /dashboard/devices
pub async fn get_dashboard_devices<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    use crate::registry::filter::{
        DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder,
    };

    let limit = pagination.limit.unwrap_or(50).min(200) as usize; 
    let page = pagination.page.unwrap_or(1);
    let offset = ((page - 1) * limit as u64) as usize; 

    let options = QueryOptions {
        filter: DeviceFilter::default(),
        sort_by: DeviceSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset { offset, limit },
    };

    let devices = state.device_registry.list(options).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    
    let total = devices.len() as u64;
    
    let response = PaginatedResponse {
        items: devices,
        total,
        page,
        limit: limit as u64,
        has_more: total > page * limit as u64,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// GET /dashboard/devices/{id}
pub async fn get_dashboard_device<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let device_id = DeviceId(ulid);

    let device = state.device_registry
        .get(device_id)
        .await
        .map_err(|e| DashboardError::Internal(e.to_string()))?
        .ok_or_else(|| DashboardError::NotFound(format!("Device {} not found", id)))?;

    Ok(Json(ApiResponse::success(device)))
}

/// POST /dashboard/devices
pub async fn create_dashboard_device<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Json(form): Json<DeviceForm>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    // Convert sensor forms to sensors
    let sensors = create_sensors_from_forms(form.sensors)?;
    
    let device = Device {
        id: DeviceId(Ulid::new()),
        kind: DeviceKind::Sensor,
        state: DeviceState::Active,
        location: H3Cell(form.location),
        manufacturer: form.manufacturer.map(|s| s.into_boxed_str()),
        provisioned_at: jiff::Timestamp::now(),
        sensors: sensors.into_boxed_slice(),
    };

    state.device_registry.register(device.clone()).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok((StatusCode::CREATED, Json(ApiResponse::success(device))))
}

/// PUT /dashboard/devices/{id}
pub async fn update_dashboard_device<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
    Json(form): Json<DeviceForm>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let device_id = DeviceId(ulid);

    // Get existing device to preserve timestamp
    let existing = state.device_registry
        .get(device_id)
        .await
        .map_err(|e| DashboardError::Internal(e.to_string()))?
        .ok_or_else(|| DashboardError::NotFound(format!("Device {} not found", id)))?;

    // Convert sensor forms to sensors
    let sensors = create_sensors_from_forms(form.sensors)?;

    let device = Device {
        id: device_id,
        kind: DeviceKind::Sensor,
        state: DeviceState::Active,
        location: H3Cell(form.location),
        manufacturer: form.manufacturer.map(|s| s.into_boxed_str()),
        provisioned_at: existing.provisioned_at,
        sensors: sensors.into_boxed_slice(),
    };

    state.device_registry.update(device_id, device.clone()).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok(Json(ApiResponse::success(device)))
}

/// DELETE /dashboard/devices/{id}
pub async fn delete_dashboard_device<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let device_id = DeviceId(ulid);

    state.device_registry.suspend(device_id).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok(Json(ApiResponse::success("Device suspended successfully")))
}

/// GET /dashboard/dispatchers
pub async fn get_dashboard_dispatchers<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };

    let limit = pagination.limit.unwrap_or(50).min(200) as usize;
    let page = pagination.page.unwrap_or(1);
    let offset = ((page - 1) * limit as u64) as usize;

    let options = QueryOptions {
        filter: DispatcherFilter::default(),
        sort_by: DispatcherSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset { offset, limit },
    };

    let dispatchers = state.dispatcher_registry.list(options).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    let total = dispatchers.len() as u64;
    
    let response = PaginatedResponse {
        items: dispatchers,
        total,
        page,
        limit: limit as u64,
        has_more: total > page * limit as u64,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// GET /dashboard/dispatchers/{id}
pub async fn get_dashboard_dispatcher<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let dispatcher_id = DispatcherId(ulid);

    let dispatcher = state.dispatcher_registry
        .get(dispatcher_id)
        .await
        .map_err(|e| DashboardError::Internal(e.to_string()))?
        .ok_or_else(|| DashboardError::NotFound(format!("Dispatcher {} not found", id)))?;

    Ok(Json(ApiResponse::success(dispatcher)))
}

/// POST /dashboard/dispatchers
pub async fn create_dashboard_dispatcher<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Json(form): Json<DispatcherForm>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let dispatcher = Dispatcher {
        id: DispatcherId(Ulid::new()),
        location: H3Cell(form.location),
        state: DispatcherState::Active,
        provisioned_at: jiff::Timestamp::now(),
    };

    state.dispatcher_registry.register(dispatcher.clone()).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok((StatusCode::CREATED, Json(ApiResponse::success(dispatcher))))
}

/// PUT /dashboard/dispatchers/{id}
pub async fn update_dashboard_dispatcher<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
    Json(form): Json<DispatcherForm>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let dispatcher_id = DispatcherId(ulid);

    // Get existing dispatcher to preserve state and timestamp
    let existing = state.dispatcher_registry
        .get(dispatcher_id)
        .await
        .map_err(|e| DashboardError::Internal(e.to_string()))?
        .ok_or_else(|| DashboardError::NotFound(format!("Dispatcher {} not found", id)))?;

    let dispatcher = Dispatcher {
        id: dispatcher_id,
        location: H3Cell(form.location),
        state: existing.state,
        provisioned_at: existing.provisioned_at,
    };

    state.dispatcher_registry.update(dispatcher_id, dispatcher.clone()).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok(Json(ApiResponse::success(dispatcher)))
}

/// DELETE /dashboard/dispatchers/{id}
pub async fn delete_dashboard_dispatcher<D, Dev>(
    State(state): State<DashboardState<D, Dev>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, DashboardError>
where
    D: DispatcherRegistry,
    Dev: DeviceRegistry,
{
    let ulid = parse_ulid(&id)?;
    let dispatcher_id = DispatcherId(ulid);

    state.dispatcher_registry.suspend(dispatcher_id).await
        .map_err(|e| DashboardError::Internal(e.to_string()))?;
    
    Ok(Json(ApiResponse::success("Dispatcher suspended successfully")))
}

/// Serve dashboard HTML
pub async fn dashboard_index() -> impl IntoResponse {
    Html(include_str!("templates/dashboard.html"))
}

/// Create dashboard router
pub fn dashboard_router<D, Dev>(dispatcher_registry: D, device_registry: Dev) -> Router<DashboardState<D, Dev>>
where
    D: DispatcherRegistry + Clone + Send + Sync + 'static,
    Dev: DeviceRegistry + Clone + Send + Sync + 'static,
{
    let state = DashboardState {
        dispatcher_registry,
        device_registry,
    };

    Router::new()
        .route("/", get(dashboard_index))
        .route("/devices", get(get_dashboard_devices::<D, Dev>))
        .route("/devices", post(create_dashboard_device::<D, Dev>))
        .route("/devices/{id}", get(get_dashboard_device::<D, Dev>))
        .route("/devices/{id}", put(update_dashboard_device::<D, Dev>))
        .route("/devices/{id}", delete(delete_dashboard_device::<D, Dev>))
        .route("/dispatchers", get(get_dashboard_dispatchers::<D, Dev>))
        .route("/dispatchers", post(create_dashboard_dispatcher::<D, Dev>))
        .route("/dispatchers/{id}", get(get_dashboard_dispatcher::<D, Dev>))
        .route("/dispatchers/{id}", put(update_dashboard_dispatcher::<D, Dev>))
        .route("/dispatchers/{id}", delete(delete_dashboard_dispatcher::<D, Dev>))
        .with_state(state)
}
