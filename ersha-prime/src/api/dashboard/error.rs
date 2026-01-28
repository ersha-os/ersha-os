use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DashboardError {
    #[error("Invalid ULID: {0}")]
    InvalidUlid(String),
    
    #[error("Resource not found: {0}")]
    NotFound(String),
    
    #[error("Invalid sensor kind: {0}")]
    InvalidSensorKind(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Internal server error")]
    Internal(String),
}

impl IntoResponse for DashboardError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            DashboardError::InvalidUlid(msg) => (StatusCode::BAD_REQUEST, msg),
            DashboardError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            DashboardError::InvalidSensorKind(msg) => (StatusCode::BAD_REQUEST, msg),
            DashboardError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            DashboardError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        #[derive(Serialize)]
        struct ErrorResponse {
            error: String,
            message: String,
        }

        (status, Json(ErrorResponse {
            error: status.to_string(),
            message,
        })).into_response()
    }
}
