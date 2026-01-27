use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::str::FromStr;
use ulid::Ulid;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};
use ersha_prime::registry::DispatcherRegistry;

use super::models::{
    ApiResponse, DispatcherCreateRequest, DispatcherResponse, DispatcherUpdateRequest,
    ListQueryParams, ListResponse,
};
use crate::registry::filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder};
use crate::AppState;

// List dispatchers with pagination
pub async fn list_dispatchers(
    State(state): State<AppState<impl crate::registry::DeviceRegistry, impl DispatcherRegistry>>,
    Query(params): Query<ListQueryParams>,
) -> impl IntoResponse {
    let options = QueryOptions {
        filter: None,
        sort_by: DispatcherSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset {
            offset: params.offset.unwrap_or(0),
            limit: params.limit.unwrap_or(50),
        },
    };

    match state.dispatcher_registry.list(options).await {
        Ok(dispatchers) => {
            let total = state.dispatcher_registry.count(None).await.unwrap_or(0);
            let responses: Vec<DispatcherResponse> = dispatchers
                .into_iter()
                .map(dispatcher_to_response)
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
                message: Some(format!("Failed to list dispatchers: {}", e)),
            }),
        ),
    }
}

// Get a specific dispatcher by ID
pub async fn get_dispatcher(
    Path(id): Path<String>,
    State(state): State<AppState<impl crate::registry::DeviceRegistry, impl DispatcherRegistry>>,
) -> impl IntoResponse {
    let dispatcher_id = match parse_dispatcher_id(&id) {
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

    match state.dispatcher_registry.get(dispatcher_id).await {
        Ok(Some(dispatcher)) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(dispatcher_to_response(dispatcher)),
                message: None,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some("Dispatcher not found".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to get dispatcher: {}", e)),
            }),
        ),
    }
}

// Create a new dispatcher
pub async fn create_dispatcher(
    State(state): State<AppState<impl crate::registry::DeviceRegistry, impl DispatcherRegistry>>,
    Json(payload): Json<DispatcherCreateRequest>,
) -> impl IntoResponse {
    let dispatcher = Dispatcher {
        id: DispatcherId(Ulid::new()),
        location: payload.location,
        state: DispatcherState::Active,
        provisioned_at: jiff::Timestamp::now(),
    };

    match state.dispatcher_registry.register(dispatcher.clone()).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                data: Some(dispatcher_to_response(dispatcher)),
                message: Some("Dispatcher created successfully".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to create dispatcher: {}", e)),
            }),
        ),
    }
}

// Update an existing dispatcher
pub async fn update_dispatcher(
    Path(id): Path<String>,
    State(state): State<AppState<impl crate::registry::DeviceRegistry, impl DispatcherRegistry>>,
    Json(payload): Json<DispatcherUpdateRequest>,
) -> impl IntoResponse {
    let dispatcher_id = match parse_dispatcher_id(&id) {
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

    match state.dispatcher_registry.get(dispatcher_id).await {
        Ok(Some(existing)) => {
            let updated = Dispatcher {
                state: payload.state.unwrap_or(existing.state),
                location: payload.location.unwrap_or(existing.location),
                ..existing
            };

            match state.dispatcher_registry.update(dispatcher_id, updated.clone()).await {
                Ok(_) => (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        data: Some(dispatcher_to_response(updated)),
                        message: Some("Dispatcher updated successfully".to_string()),
                    }),
                ),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: Some(format!("Failed to update dispatcher: {}", e)),
                    }),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some("Dispatcher not found".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to get dispatcher: {}", e)),
            }),
        ),
    }
}

// Delete (suspend) a dispatcher
pub async fn delete_dispatcher(
    Path(id): Path<String>,
    State(state): State<AppState<impl crate::registry::DeviceRegistry, impl DispatcherRegistry>>,
) -> impl IntoResponse {
    let dispatcher_id = match parse_dispatcher_id(&id) {
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

    match state.dispatcher_registry.suspend(dispatcher_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::<()> {
                success: true,
                data: None,
                message: Some("Dispatcher suspended successfully".to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("Failed to suspend dispatcher: {}", e)),
            }),
        ),
    }
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
