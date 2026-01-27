use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::str::FromStr;
use ulid::Ulid;

use crate::registry::DispatcherRegistry;
use ersha_core::{Dispatcher, DispatcherId, DispatcherState};

use super::models::{
    ApiResponse, DispatcherCreateRequest, DispatcherResponse, DispatcherUpdateRequest,
    ListQueryParams, ListResponse,
};
use crate::registry::filter::{
    DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
};

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
        provisioned_at: dispatcher.provisioned_at.to_string(),
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
fn success_response<T: serde::Serialize>(
    status: StatusCode,
    data: T,
    message: Option<String>,
) -> Response {
    let api_response = ApiResponse {
        success: true,
        data: Some(data),
        message,
    };
    (status, Json(api_response)).into_response()
}

// List dispatchers with pagination
pub async fn list_dispatchers<DR, DisR>(
    State(state): State<crate::AppState<DR, DisR>>,
    Query(params): Query<ListQueryParams>,
) -> Response
where
    DR: Send + Sync,
    DisR: DispatcherRegistry + Send + Sync,
{
    let options = QueryOptions {
        filter: DispatcherFilter::default(),
        sort_by: DispatcherSortBy::ProvisionAt,
        sort_order: SortOrder::Desc,
        pagination: Pagination::Offset {
            offset: params.offset.unwrap_or(0),
            limit: params.limit.unwrap_or(50),
        },
    };

    let list_result = state.dispatcher_registry.list(options).await;
    let count_result = state.dispatcher_registry.count(None).await;

    match (list_result, count_result) {
        (Ok(dispatchers), Ok(total)) => {
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

            success_response(StatusCode::OK, response, None)
        }
        (Err(e), _) | (_, Err(e)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list dispatchers: {}", e),
        ),
    }
}

// Get a specific dispatcher by ID
pub async fn get_dispatcher<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
) -> Response
where
    DR: Send + Sync,
    DisR: DispatcherRegistry + Send + Sync,
{
    let dispatcher_id = match parse_dispatcher_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.dispatcher_registry.get(dispatcher_id).await {
        Ok(Some(dispatcher)) => {
            success_response(StatusCode::OK, dispatcher_to_response(dispatcher), None)
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Dispatcher not found".to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get dispatcher: {}", e),
        ),
    }
}

// Create a new dispatcher
pub async fn create_dispatcher<DR, DisR>(
    State(state): State<crate::AppState<DR, DisR>>,
    Json(payload): Json<DispatcherCreateRequest>,
) -> Response
where
    DR: Send + Sync,
    DisR: DispatcherRegistry + Send + Sync,
{
    let dispatcher = Dispatcher {
        id: DispatcherId(Ulid::new()),
        location: payload.location,
        state: DispatcherState::Active,
        provisioned_at: jiff::Timestamp::now(),
    };

    match state.dispatcher_registry.register(dispatcher.clone()).await {
        Ok(_) => success_response(
            StatusCode::CREATED,
            dispatcher_to_response(dispatcher),
            Some("Dispatcher created successfully".to_string()),
        ),
        Err(e) => error_response(
            StatusCode::BAD_REQUEST,
            format!("Failed to create dispatcher: {}", e),
        ),
    }
}

// Update an existing dispatcher
pub async fn update_dispatcher<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
    Json(payload): Json<DispatcherUpdateRequest>,
) -> Response
where
    DR: Send + Sync,
    DisR: DispatcherRegistry + Send + Sync,
{
    let dispatcher_id = match parse_dispatcher_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.dispatcher_registry.get(dispatcher_id).await {
        Ok(Some(existing)) => {
            let updated = Dispatcher {
                state: payload.state.unwrap_or(existing.state),
                location: payload.location.unwrap_or(existing.location),
                ..existing
            };

            match state
                .dispatcher_registry
                .update(dispatcher_id, updated.clone())
                .await
            {
                Ok(_) => success_response(
                    StatusCode::OK,
                    dispatcher_to_response(updated),
                    Some("Dispatcher updated successfully".to_string()),
                ),
                Err(e) => error_response(
                    StatusCode::BAD_REQUEST,
                    format!("Failed to update dispatcher: {}", e),
                ),
            }
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Dispatcher not found".to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get dispatcher: {}", e),
        ),
    }
}

// Delete (suspend) a dispatcher
pub async fn delete_dispatcher<DR, DisR>(
    Path(id): Path<String>,
    State(state): State<crate::AppState<DR, DisR>>,
) -> Response
where
    DR: Send + Sync,
    DisR: DispatcherRegistry + Send + Sync,
{
    let dispatcher_id = match parse_dispatcher_id(&id) {
        Ok(id) => id,
        Err(message) => {
            return error_response(StatusCode::BAD_REQUEST, message);
        }
    };

    match state.dispatcher_registry.suspend(dispatcher_id).await {
        Ok(_) => success_response(
            StatusCode::OK,
            (),
            Some("Dispatcher suspended successfully".to_string()),
        ),
        Err(e) => error_response(
            StatusCode::BAD_REQUEST,
            format!("Failed to suspend dispatcher: {}", e),
        ),
    }
}
