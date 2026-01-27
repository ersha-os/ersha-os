use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::str::FromStr;
use ulid::Ulid;

use ersha_core::{Device, DeviceId, DeviceKind, DeviceState, H3Cell, Sensor, SensorId, SensorKind};
use ersha_prime::registry::DeviceRegistry;

use super::models::{
    ApiResponse, DeviceCreateRequest, DeviceResponse, DeviceUpdateRequest, ListQueryParams,
    ListResponse, SensorCreateRequest, SensorResponse,
};
use crate::registry::filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder};
use crate::AppState;

// List devices with pagination and filtering
pub async fn list_devices(
    State(state): State<AppState<impl DeviceRegistry, impl crate::registry::DispatcherRegistry>>,
    Query(params): Query<ListQueryParams>,
) -> impl IntoResponse {
    let options = QueryOptions {
        filter: None, // Start with no filters
        sort_by: DeviceSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset {
            offset: params.offset.unwrap_or(0),
            limit: params.limit.unwrap_or(50),
        },
    };

    match state.device_registry.list(options).await {
        Ok(devices) => {
            let total = state.device_registry.count(None).await.unwrap_or(0);
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
            
            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(response),
                    message: None,
                }),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to list devices: {}", e)),
            }),
        ),
    }
}

// Get a specific device by ID
pub async fn get_device(
    Path(id): Path<String>,
    State(state): State<AppState<impl DeviceRegistry, impl crate::registry::DispatcherRegistry>>,
) -> impl IntoResponse {
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: Some(message),
                }),
            );
        }
    };

    match state.device_registry.get(device_id).await {
        Ok(Some(device)) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(device_to_response(device)),
                message: None,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some("Device not found".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to get device: {}", e)),
            }),
        ),
    }
}

// Create a new device
pub async fn create_device(
    State(state): State<AppState<impl DeviceRegistry, impl crate::registry::DispatcherRegistry>>,
    Json(payload): Json<DeviceCreateRequest>,
) -> impl IntoResponse {
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
        Ok(_) => (
            StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                data: Some(device_to_response(device)),
                message: Some("Device created successfully".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to create device: {}", e)),
            }),
        ),
    }
}

// Update an existing device
pub async fn update_device(
    Path(id): Path<String>,
    State(state): State<AppState<impl DeviceRegistry, impl crate::registry::DispatcherRegistry>>,
    Json(payload): Json<DeviceUpdateRequest>,
) -> impl IntoResponse {
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: Some(message),
                }),
            );
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
                Ok(_) => (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        data: Some(device_to_response(updated)),
                        message: Some("Device updated successfully".to_string()),
                    }),
                ),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to update device: {}", e)),
                    }),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some("Device not found".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to get device: {}", e)),
            }),
        ),
    }
}

// Delete (suspend) a device
pub async fn delete_device(
    Path(id): Path<String>,
    State(state): State<AppState<impl DeviceRegistry, impl crate::registry::DispatcherRegistry>>,
) -> impl IntoResponse {
    let device_id = match parse_device_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: Some(message),
                }),
            );
        }
    };

    match state.device_registry.suspend(device_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::<()> {
                success: true,
                data: None,
                message: Some("Device suspended successfully".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to suspend device: {}", e)),
            }),
        ),
    }
}

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
        provisioned_at: device.provisioned_at.to_rfc3339(),
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
