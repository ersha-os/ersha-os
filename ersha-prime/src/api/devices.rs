use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::str::FromStr;
use ulid::Ulid;

use ersha_core::{Device, DeviceId, DeviceState, Sensor, SensorId};
use crate::registry::DeviceRegistry;

use super::models::{
    ApiResponse, DeviceCreateRequest, DeviceResponse, DeviceUpdateRequest, ListQueryParams,
    ListResponse, SensorCreateRequest, SensorResponse,
};
use crate::registry::filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder};

// Helper function to parse DeviceId from string
fn parse_device_id(id: &str) -> Result<DeviceId, String> {
    Ulid::from_str(id)
        .map(DeviceId)
        .map_err(|_| "Invalid device ID format. Expected ULID.".to_string())
}

// Convert Device to DeviceResponse
fn device_to_response(device: Device) -> DeviceResponse {
    DeviceResponse {
        id: device.id.0.to_string(),
        kind: device.kind,
        state: device.state,
        location: device.location,
        manufacturer: device.manufacturer.map(|s| s.to_string()),
        provisioned_at: device.provisioned_at.to_string(),
        sensors: device
            .sensors
            .into_vec()
            .into_iter()
            .map(|s| SensorResponse {
                id: s.id.0.to_string(),
                kind: s.kind,
                metric: s.metric.into(),
            })
            .collect(),
    }
}

// Helper to create error response
fn error_response(status: StatusCode, message: String) -> Response {
    let api_response = ApiResponse::<()> {
        success: false,
        data: None,
        message: Some(message),
    };
    (status, Json(api_response)).into_response()
}

// Helper to create success response
fn success_response<T: serde::Serialize>(status: StatusCode, data: T, message: Option<String>) -> Response {
    let api_response = ApiResponse {
        success: true,
        data: Some(data),
        message,
    };
    (status, Json(api_response)).into_response()
}

// List devices with pagination and filtering
pub async fn list_devices<DR, DisR>(
    State(state): State<crate::AppState<DR, DisR>>,
    Query(params): Query<ListQueryParams>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let options = QueryOptions {
        filter: DeviceFilter::default(),
        sort_by: DeviceSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset {
            offset: params.offset.unwrap_or(0),
            limit: params.limit.unwrap_or(50),
        },
    };

    let list_result = state.device_registry.list(options).await;
    let count_result = state.device_registry.count(None).await;
    
    match (list_result, count_result) {
        (Ok(devices), Ok(total)) => {
            let responses: Vec<DeviceResponse> = devices
                .into_iter()
                .map(device_to_response)
                .collect();
            
            let response = ListResponse {
                items: responses,
                total,
                page: Some(params.offset.unwrap_or(0) / params.limit.unwrap_or(50)),
                per_page: params.limit,
                has_more: total > params.offset.unwrap_or(0) + params.limit.unwrap_or(50),
            };
            
            success_response(StatusCode::OK, response, None)
        }
        (Err(e), _) | (_, Err(e)) => {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list devices: {}", e)
            )
        }
    }
}

// Get a specific device by ID
pub async fn get_device<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.device_registry.get(device_id).await {
        Ok(Some(device)) => {
            success_response(StatusCode::OK, device_to_response(device), None)
        }
        Ok(None) => {
            error_response(StatusCode::NOT_FOUND, "Device not found".to_string())
        }
        Err(e) => {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get device: {}", e)
            )
        }
    }
}

// Create a new device
pub async fn create_device<DR, DisR>(
    State(state): State<crate::AppState<DR, DisR>>,
    Json(payload): Json<DeviceCreateRequest>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let device = Device {
        id: DeviceId(Ulid::new()),
        kind: payload.kind,
        state: DeviceState::Active,
        location: payload.location,
        manufacturer: payload.manufacturer.map(|s| s.into_boxed_str()),
        provisioned_at: jiff::Timestamp::now(),
        sensors: payload
            .sensors
            .into_iter()
            .map(|s| Sensor {
                id: SensorId(Ulid::new()),
                kind: s.kind,
                metric: s.metric.into(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    };

    match state.device_registry.register(device.clone()).await {
        Ok(_) => {
            success_response(
                StatusCode::CREATED, 
                device_to_response(device), 
                Some("Device created successfully".to_string())
            )
        }
        Err(e) => {
            error_response(
                StatusCode::BAD_REQUEST,
                format!("Failed to create device: {}", e)
            )
        }
    }
}

// Update an existing device
pub async fn update_device<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
    Json(payload): Json<DeviceUpdateRequest>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.device_registry.get(device_id).await {
        Ok(Some(existing)) => {
            let updated = Device {
                kind: payload.kind.unwrap_or(existing.kind),
                state: payload.state.unwrap_or(existing.state),
                location: payload.location.unwrap_or(existing.location),
                manufacturer: match payload.manufacturer {
                    Some(man) => man.map(|s| s.into_boxed_str()),
                    None => existing.manufacturer,
                },
                ..existing
            };

            match state.device_registry.update(device_id, updated.clone()).await {
                Ok(_) => {
                    success_response(
                        StatusCode::OK,
                        device_to_response(updated),
                        Some("Device updated successfully".to_string())
                    )
                }
                Err(e) => {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        format!("Failed to update device: {}", e)
                    )
                }
            }
        }
        Ok(None) => {
            error_response(StatusCode::NOT_FOUND, "Device not found".to_string())
        }
        Err(e) => {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get device: {}", e)
            )
        }
    }
}

// Delete (suspend) a device
pub async fn delete_device<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.device_registry.suspend(device_id).await {
        Ok(_) => {
            success_response(
                StatusCode::OK,
                (),
                Some("Device suspended successfully".to_string())
            )
        }
        Err(e) => {
            error_response(
                StatusCode::BAD_REQUEST,
                format!("Failed to suspend device: {}", e)
            )
        }
    }
}

// Add a sensor to a device
pub async fn add_sensor<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
    Json(payload): Json<SensorCreateRequest>,
) -> Response
where
    DR: DeviceRegistry + Send + Sync,
    DisR: Send + Sync,
{
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    let sensor = Sensor {
        id: SensorId(Ulid::new()),
        kind: payload.kind,
        metric: payload.metric.into(),
    };

    match state.device_registry.add_sensor(device_id, sensor).await {
        Ok(_) => {
            success_response(
                StatusCode::CREATED,
                (),
                Some("Sensor added successfully".to_string())
            )
        }
        Err(e) => {
            error_response(
                StatusCode::BAD_REQUEST,
                format!("Failed to add sensor: {}", e)
            )
        }
    }
}
