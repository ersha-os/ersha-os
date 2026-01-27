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
