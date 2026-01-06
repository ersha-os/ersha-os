use ulid::Ulid;

type BoxStr = Box<str>;
type BoxList<T> = Box<[T]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(pub Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadingId(pub Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusId(pub Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatcherId(pub Ulid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct H3Cell(pub u64);

pub struct Device {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub state: DeviceState,
    pub location: H3Cell,
    pub manufacturer: Option<BoxStr>,
    pub provisioned_at: jiff::Timestamp,
}

pub enum DeviceKind {
    Sensor,
}

pub enum DeviceState {
    Active,
    Suspended,
}

pub struct SensorReading {
    pub id: ReadingId,
    pub device_id: DeviceId,
    pub dispatcher_id: DispatcherId,
    pub metric: SensorMetric,
    pub quality: ReadingQuality,
    pub location: H3Cell,
    pub timestamp: jiff::Timestamp,
}

pub struct ReadingQuality {
    pub status: QualityStatus,
    pub confidence: f32,
}

pub enum QualityStatus {
    Ok,
    Suspect,
    Bad,
}

pub struct SensorMetric {
    pub kind: SensorMetricKind,
    pub value: f64,
    pub unit: MetricUnit,
}

pub enum MetricUnit {
    Percent,
    Celsius,
    Mm,
}

pub enum SensorMetricKind {
    SoilMoisture,
    SoilTemp,
    AirTemp,
    Humidity,
    Rainfall,
}

pub struct DeviceStatus {
    pub id: StatusId,
    pub device_id: DeviceId,
    pub dispatcher_id: DispatcherId,
    pub battery_percent: u8,
    pub uptime_seconds: u64,
    pub signal_rssi: i64,
    pub errors: BoxList<DeviceError>,
    pub timestamp: jiff::Timestamp,
}

pub struct DeviceError {
    pub code: DeviceErrorCode,
    pub message: Option<BoxStr>,
}

pub enum DeviceErrorCode {
    LowBattery,
    SensorFault,
    RadioFault,
    Unknown,
}
