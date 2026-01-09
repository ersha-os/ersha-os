use serde::{Deserialize, Serialize};
use ulid::Ulid;

// We use `Box<str>` and `Box<[T]>` for structures that don't need to be
// dynamically sized. This helps us keep allocations compact and avoid
// accidental cloning of large values.
type BoxStr = Box<str>;
type BoxList<T> = Box<[T]>;

/// Unique identifier for an edge device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Ulid);

/// Unique identifier for a telemetry reading event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadingId(pub Ulid);

/// Unique identifier for a device status report event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatusId(pub Ulid);

/// Unique identifier for a dispatcher device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DispatcherId(pub Ulid);

/// Unique identifier for an upload batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(pub Ulid);

/// Unique identifier for a sensor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SensorId(pub Ulid);

/// H3 cell index (hex-like 64-bit integer) representing a spatial cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct H3Cell(pub u64);

/// Percentage value in the range 0–100 (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Percentage(pub u8);

/// A registered edge device in the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Stable identity of this device.
    pub id: DeviceId,
    /// Type of the device.
    pub kind: DeviceKind,
    /// Operational state of device.
    pub state: DeviceState,
    /// Canonical location cell for the device.
    pub location: H3Cell,
    /// Manufacturer or vendor string.
    pub manufacturer: Option<BoxStr>,
    /// Provisioning timestamp.
    pub provisioned_at: jiff::Timestamp,
    /// Sensors attached to this device.
    pub sensors: BoxList<Sensor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sensor {
    pub id: SensorId,
    pub metric: SensorMetric,
    pub kind: SensorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStatus {
    pub sensor_id: SensorId,
    pub state: SensorState,
    pub last_reading: Option<jiff::Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorState {
    Active,
    Faulty,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorKind {
    SoilMoisture,
    SoilTemp,
    AirTemp,
    Humidity,
    Rainfall,
}

/// Device classification.
/// Actuators can be added later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceKind {
    Sensor,
}

/// Device state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceState {
    /// Device is permitted to upload telemetry.
    Active,
    /// Device is blocked (e.g., compromised, decommissioned, etc.).
    Suspended,
}

/// A single sensor reading emitted by an edge device and forwarded by a dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Unique id for this reading.
    pub id: ReadingId,
    /// Source device that generated this reading.
    pub device_id: DeviceId,
    /// Dispatcher that forwarded this reading to central.
    pub dispatcher_id: DispatcherId,
    /// The measured quantity and value.
    pub metric: SensorMetric,
    /// H3 cell where the reading was taken.
    pub location: H3Cell,
    /// Quality of this reading.
    pub confidence: Percentage,
    /// Timestamp of the reading event.
    pub timestamp: jiff::Timestamp,
    /// The specific sensor that produced this reading
    pub sensor_id: SensorId,
}

/// Supported sensor metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorMetric {
    /// Soil moisture as a percentage.
    SoilMoisture { value: Percentage },
    /// Soil temperature in degrees Celsius.
    SoilTemp { value: f64 },
    /// Air temperature in degrees Celsius.
    AirTemp { value: f64 },
    /// Relative humidity as a percentage.
    Humidity { value: Percentage },
    /// Rainfall in millimeters.
    Rainfall { value: f64 },
}

/// Units used by metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricUnit {
    /// Percent (%) values.
    Percent,
    /// Degrees Celsius (°C).
    Celsius,
    /// Millimeters (mm).
    Mm,
}

/// A status report emitted by a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// Unique id for this status record.
    pub id: StatusId,
    /// Source device that generated this status report.
    pub device_id: DeviceId,
    /// Dispatcher that forwarded this status report.
    pub dispatcher_id: DispatcherId,
    /// Battery charge level expressed as a percentage.
    pub battery_percent: Percentage,
    /// Device uptime (seconds since last reboot).
    pub uptime_seconds: u64,
    /// Received signal strength indicator (RSSI).
    pub signal_rssi: i16,
    /// Any errors reported by device firmware.
    pub errors: BoxList<DeviceError>,
    /// Timestamp when status was captured.
    pub timestamp: jiff::Timestamp,
    /// The status of each sensor attached to this device
    pub sensor_statuses: BoxList<SensorStatus>,
}

/// A structured error from a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceError {
    /// Canonical error category.
    pub code: DeviceErrorCode,
    /// Optional human-readable message from firmware.
    pub message: Option<BoxStr>,
}

/// Device error codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceErrorCode {
    LowBattery,
    SensorFault,
    RadioFault,
    Unknown,
}

/// A registered dispatcher in the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispatcher {
    /// Stable identity of this dispatcher.
    pub id: DispatcherId,
    /// Dispatcher location cell.
    pub location: H3Cell,
    /// Operational state.
    pub state: DispatcherState,
    /// Provisioning timestamp.
    pub provisioned_at: jiff::Timestamp,
}

/// Dispatcher State
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DispatcherState {
    /// Dispatcher is permitted to upload data.
    Active,
    /// Dispatcher is blocked (e.g., compromised, decommissioned).
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUploadRequest {
    /// Unique id for this batch.
    pub id: BatchId,
    /// Dispatcher that created and is uploading this batch.
    pub dispatcher_id: DispatcherId,
    /// Telemetry readings included in this batch.
    pub readings: BoxList<SensorReading>,
    /// Device status records included in this batch.
    pub statuses: BoxList<DeviceStatus>,
    /// Timestamp when the batch was created by dispatcher.
    pub timestamp: jiff::Timestamp,
}

// Acknowledgement of a batch upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchUploadResponse {
    pub id: BatchId,
}
