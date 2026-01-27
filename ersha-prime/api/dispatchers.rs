use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::str::FromStr;
use ulid::Ulid;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};

use super::models::{
    ApiResponse, DispatcherCreateRequest, DispatcherResponse, DispatcherUpdateRequest,
    ListQueryParams, ListResponse,
};
use crate::AppState;

// List dispatchers with pagination
pub async fn list_dispatchers(
    State(_state): State<AppState<impl crate::registry::DeviceRegistry, impl ersha_prime::registry::DispatcherRegistry>>,
    Query(_params): Query<ListQueryParams>,
) -> impl IntoResponse {
    // TODO: Implement list functionality
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: Some("Not implemented".to_string()),
        }),
    )
}

// Get a specific dispatcher by ID
pub async fn get_dispatcher(
    Path(_id): Path<String>,
    State(_state): State<AppState<impl crate::registry::DeviceRegistry, impl ersha_prime::registry::DispatcherRegistry>>,
) -> impl IntoResponse {
    // TODO: Implement get functionality
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: Some("Not implemented".to_string()),
        }),
    )
}

// Create a new dispatcher
pub async fn create_dispatcher(
    State(_state): State<AppState<impl crate::registry::DeviceRegistry, impl ersha_prime::registry::DispatcherRegistry>>,
    Json(_payload): Json<DispatcherCreateRequest>,
) -> impl IntoResponse {
    // TODO: Implement create functionality
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: Some("Not implemented".to_string()),
        }),
    )
}

// Update an existing dispatcher
pub async fn update_dispatcher(
    Path(_id): Path<String>,
    State(_state): State<AppState<impl crate::registry::DeviceRegistry, impl ersha_prime::registry::DispatcherRegistry>>,
    Json(_payload): Json<DispatcherUpdateRequest>,
) -> impl IntoResponse {
    // TODO: Implement update functionality
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: Some("Not implemented".to_string()),
        }),
    )
}

// Delete (suspend) a dispatcher
pub async fn delete_dispatcher(
    Path(_id): Path<String>,
    State(_state): State<AppState<impl crate::registry::DeviceRegistry, impl ersha_prime::registry::DispatcherRegistry>>,
) -> impl IntoResponse {
    // TODO: Implement delete functionality
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: Some("Not implemented".to_string()),
        }),
    )
}

// Helper function to parse DispatcherId from string
fn parse_dispatcher_id(id: &str) -> Result<DispatcherId, String> {
    Ulid::from_str(id)
        .map(DispatcherId)
        .map_err(|_| "Invalid dispatcher ID format. Expected ULID.".to_string())
}

// Convert Dispatcher to DispatcherResponse
fn dispatcher_to_response(dispatcher: Dispatcher) -> DispatcherResponse {
    DispatcherResponse {
        id: dispatcher.id.0.to_string(),
        state: dispatcher.state,
        location: dispatcher.location,
        provisioned_at: dispatcher.provisioned_at.to_rfc3339(),
    }
}
