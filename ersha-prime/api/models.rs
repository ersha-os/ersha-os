use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ersha_core::{
    DeviceKind, DeviceState, DispatcherState, H3Cell, 
    SensorKind, SensorMetric, Percentage, SensorState, DeviceErrorCode
};
use jiff::Timestamp;
use ulid::Ulid;
use ordered_float::NotNan;

// Device Request/Response Models
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCreateRequest {
    pub kind: DeviceKind,
    pub location: H3Cell,
    pub manufacturer: Option<String>,
    pub sensors: Vec<SensorCreateRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorCreateRequest {
    pub kind: SensorKind,
    pub metric: SensorMetricRequest,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SensorMetricRequest {
    SoilMoisture { value: u8 },
    SoilTemp { value: f64 },
    AirTemp { value: f64 },
    Humidity { value: u8 },
    Rainfall { value: f64 },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceUpdateRequest {
    pub kind: Option<DeviceKind>,
    pub state: Option<DeviceState>,
    pub location: Option<H3Cell>,
    pub manufacturer: Option<Option<String>>, // Optional<Option> to allow null
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceResponse {
    pub id: String,
    pub kind: DeviceKind,
    pub state: DeviceState,
    pub location: H3Cell,
    pub manufacturer: Option<String>,
    pub provisioned_at: String,
    pub sensors: Vec<SensorResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorResponse {
    pub id: String,
    pub kind: SensorKind,
    pub metric: SensorMetricResponse,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SensorMetricResponse {
    SoilMoisture { value: u8 },
    SoilTemp { value: f64 },
    AirTemp { value: f64 },
    Humidity { value: u8 },
    Rainfall { value: f64 },
}

// Dispatcher Request/Response Models
#[derive(Debug, Serialize, Deserialize)]
pub struct DispatcherCreateRequest {
    pub location: H3Cell,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatcherUpdateRequest {
    pub state: Option<DispatcherState>,
    pub location: Option<H3Cell>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DispatcherResponse {
    pub id: String,
    pub state: DispatcherState,
    pub location: H3Cell,
    pub provisioned_at: String,
}

// Device Status Models
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceStatusResponse {
    pub id: String,
    pub device_id: String,
    pub dispatcher_id: String,
    pub battery_percent: u8,
    pub uptime_seconds: u64,
    pub signal_rssi: i16,
    pub errors: Vec<DeviceErrorResponse>,
    pub timestamp: String,
    pub sensor_statuses: Vec<SensorStatusResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceErrorResponse {
    pub code: DeviceErrorCode,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorStatusResponse {
    pub sensor_id: String,
    pub state: SensorState,
    pub last_reading: Option<String>,
}

// Common Response Models
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub has_more: bool,
}

// Query Parameters
#[derive(Debug, Deserialize, Default)]
pub struct ListQueryParams {
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
}

// Helper conversions
impl From<SensorMetricRequest> for ersha_core::SensorMetric {
    fn from(req: SensorMetricRequest) -> Self {
        match req {
            SensorMetricRequest::SoilMoisture { value } => 
                ersha_core::SensorMetric::SoilMoisture { 
                    value: Percentage(value) 
                },
            SensorMetricRequest::SoilTemp { value } => 
                ersha_core::SensorMetric::SoilTemp { 
                    value: NotNan::new(value).expect("Invalid NaN value") 
                },
            SensorMetricRequest::AirTemp { value } => 
                ersha_core::SensorMetric::AirTemp { 
                    value: NotNan::new(value).expect("Invalid NaN value") 
                },
            SensorMetricRequest::Humidity { value } => 
                ersha_core::SensorMetric::Humidity { 
                    value: Percentage(value) 
                },
            SensorMetricRequest::Rainfall { value } => 
                ersha_core::SensorMetric::Rainfall { 
                    value: NotNan::new(value).expect("Invalid NaN value") 
                },
        }
    }
}

impl From<ersha_core::SensorMetric> for SensorMetricResponse {
    fn from(metric: ersha_core::SensorMetric) -> Self {
        match metric {
            ersha_core::SensorMetric::SoilMoisture { value } => 
                SensorMetricResponse::SoilMoisture { value: value.0 },
            ersha_core::SensorMetric::SoilTemp { value } => 
                SensorMetricResponse::SoilTemp { value: value.into_inner() },
            ersha_core::SensorMetric::AirTemp { value } => 
                SensorMetricResponse::AirTemp { value: value.into_inner() },
            ersha_core::SensorMetric::Humidity { value } => 
                SensorMetricResponse::Humidity { value: value.0 },
            ersha_core::SensorMetric::Rainfall { value } => 
                SensorMetricResponse::Rainfall { value: value.into_inner() },
        }
    }
}
