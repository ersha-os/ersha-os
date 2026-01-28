use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceForm {
    pub location: u64,
    pub manufacturer: Option<String>,
    pub sensors: Vec<SensorForm>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorForm {
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatcherForm {
    pub location: u64,
}

// Response models
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub message: Option<String>,
    pub success: bool,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            data: Some(data),
            message: None,
            success: true,
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            data: None,
            message: Some(message),
            success: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub has_more: bool,
}

// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}
