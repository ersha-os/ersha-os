use ersha_core::{
    DeviceErrorCode, DeviceId, DeviceKind, DeviceState, DispatcherId, DispatcherState, H3Cell,
    ReadingId, SensorId, StatusId,
};

use jiff;
use std::ops::RangeInclusive;
use ulid::Ulid;

/// Enum variant discriminator for SensorMetric, used for filtering by metric type without values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorMetricType {
    SoilMoisture,
    SoilTemp,
    AirTemp,
    Humidity,
    Rainfall,
}

pub enum DeviceSortBy {
    State,
    Manufacturer,
    ProvisionAt,
    SensorCount,
}

pub enum DispatcherSortBy {
    ProvisionAt,
}

pub enum SortOrder {
    Asc,
    Desc,
}

pub enum Pagination {
    Offset { offset: usize, limit: usize },
    Cursor { after: Option<Ulid>, limit: usize },
}

pub struct QueryOptions<F, S> {
    pub filter: F,
    pub sort_by: S,
    pub sort_order: SortOrder,
    pub pagination: Pagination,
}

#[derive(Default)]
pub struct DeviceFilter {
    pub ids: Option<Vec<DeviceId>>,
    pub states: Option<Vec<DeviceState>>,
    pub kinds: Option<Vec<DeviceKind>>,
    pub locations: Option<Vec<H3Cell>>,
    pub provisioned_after: Option<jiff::Timestamp>,
    pub provisioned_before: Option<jiff::Timestamp>,
    pub sensor_count: Option<RangeInclusive<usize>>,
    pub manufacturer_pattern: Option<String>,
}

impl DeviceFilter {
    pub fn builder() -> DeviceFilterBuilder {
        DeviceFilterBuilder::new()
    }
}

#[derive(Default)]
pub struct DeviceFilterBuilder {
    filter: DeviceFilter,
}

impl DeviceFilterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids<I>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = DeviceId>,
    {
        self.filter.ids = Some(ids.into_iter().collect());
        self
    }

    pub fn states<I>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = DeviceState>,
    {
        self.filter.states = Some(states.into_iter().collect());
        self
    }

    pub fn kinds<I>(mut self, kinds: I) -> Self
    where
        I: IntoIterator<Item = DeviceKind>,
    {
        self.filter.kinds = Some(kinds.into_iter().collect());
        self
    }

    pub fn locations<I>(mut self, locations: I) -> Self
    where
        I: IntoIterator<Item = H3Cell>,
    {
        self.filter.locations = Some(locations.into_iter().collect());
        self
    }

    pub fn provisioned_after(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.provisioned_after = Some(ts);
        self
    }

    pub fn provisioned_before(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.provisioned_before = Some(ts);
        self
    }

    pub fn sensor_count(mut self, range: RangeInclusive<usize>) -> Self {
        self.filter.sensor_count = Some(range);
        self
    }

    pub fn manufacturer_pattern<S>(mut self, pattern: S) -> Self
    where
        S: Into<String>,
    {
        self.filter.manufacturer_pattern = Some(pattern.into());
        self
    }

    pub fn build(self) -> DeviceFilter {
        self.filter
    }
}

#[derive(Default, Clone)]
pub struct DispatcherFilter {
    pub states: Option<Vec<DispatcherState>>,
    pub locations: Option<Vec<H3Cell>>,
}

impl DispatcherFilter {
    pub fn builder() -> DispatcherFilterBuilder {
        DispatcherFilterBuilder::new()
    }
}

#[derive(Default)]
pub struct DispatcherFilterBuilder {
    filter: DispatcherFilter,
}

impl DispatcherFilterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn states<I>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = DispatcherState>,
    {
        self.filter.states = Some(states.into_iter().collect());
        self
    }

    pub fn locations<I>(mut self, locations: I) -> Self
    where
        I: IntoIterator<Item = H3Cell>,
    {
        self.filter.locations = Some(locations.into_iter().collect());
        self
    }

    pub fn build(self) -> DispatcherFilter {
        self.filter
    }
}

pub enum ReadingSortBy {
    Timestamp,
    Confidence,
    DeviceId,
}

#[derive(Default)]
pub struct ReadingFilter {
    pub ids: Option<Vec<ReadingId>>,
    pub device_ids: Option<Vec<DeviceId>>,
    pub sensor_ids: Option<Vec<SensorId>>,
    pub dispatcher_ids: Option<Vec<DispatcherId>>,
    pub metric_types: Option<Vec<SensorMetricType>>,
    pub locations: Option<Vec<H3Cell>>,
    pub timestamp_after: Option<jiff::Timestamp>,
    pub timestamp_before: Option<jiff::Timestamp>,
    pub confidence_range: Option<RangeInclusive<u8>>,
}

impl ReadingFilter {
    pub fn builder() -> ReadingFilterBuilder {
        ReadingFilterBuilder::new()
    }
}

#[derive(Default)]
pub struct ReadingFilterBuilder {
    filter: ReadingFilter,
}

impl ReadingFilterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids<I>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = ReadingId>,
    {
        self.filter.ids = Some(ids.into_iter().collect());
        self
    }

    pub fn device_ids<I>(mut self, device_ids: I) -> Self
    where
        I: IntoIterator<Item = DeviceId>,
    {
        self.filter.device_ids = Some(device_ids.into_iter().collect());
        self
    }

    pub fn sensor_ids<I>(mut self, sensor_ids: I) -> Self
    where
        I: IntoIterator<Item = SensorId>,
    {
        self.filter.sensor_ids = Some(sensor_ids.into_iter().collect());
        self
    }

    pub fn dispatcher_ids<I>(mut self, dispatcher_ids: I) -> Self
    where
        I: IntoIterator<Item = DispatcherId>,
    {
        self.filter.dispatcher_ids = Some(dispatcher_ids.into_iter().collect());
        self
    }

    pub fn metric_types<I>(mut self, metric_types: I) -> Self
    where
        I: IntoIterator<Item = SensorMetricType>,
    {
        self.filter.metric_types = Some(metric_types.into_iter().collect());
        self
    }

    pub fn locations<I>(mut self, locations: I) -> Self
    where
        I: IntoIterator<Item = H3Cell>,
    {
        self.filter.locations = Some(locations.into_iter().collect());
        self
    }

    pub fn timestamp_after(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.timestamp_after = Some(ts);
        self
    }

    pub fn timestamp_before(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.timestamp_before = Some(ts);
        self
    }

    pub fn confidence_range(mut self, range: RangeInclusive<u8>) -> Self {
        self.filter.confidence_range = Some(range);
        self
    }

    pub fn build(self) -> ReadingFilter {
        self.filter
    }
}

pub enum DeviceStatusSortBy {
    Timestamp,
    BatteryPercent,
    DeviceId,
}

#[derive(Default)]
pub struct DeviceStatusFilter {
    pub ids: Option<Vec<StatusId>>,
    pub device_ids: Option<Vec<DeviceId>>,
    pub dispatcher_ids: Option<Vec<DispatcherId>>,
    pub timestamp_after: Option<jiff::Timestamp>,
    pub timestamp_before: Option<jiff::Timestamp>,
    pub battery_range: Option<RangeInclusive<u8>>,
    pub has_errors: Option<bool>,
    pub error_codes: Option<Vec<DeviceErrorCode>>,
}

impl DeviceStatusFilter {
    pub fn builder() -> DeviceStatusFilterBuilder {
        DeviceStatusFilterBuilder::new()
    }
}

#[derive(Default)]
pub struct DeviceStatusFilterBuilder {
    filter: DeviceStatusFilter,
}

impl DeviceStatusFilterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids<I>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = StatusId>,
    {
        self.filter.ids = Some(ids.into_iter().collect());
        self
    }

    pub fn device_ids<I>(mut self, device_ids: I) -> Self
    where
        I: IntoIterator<Item = DeviceId>,
    {
        self.filter.device_ids = Some(device_ids.into_iter().collect());
        self
    }

    pub fn dispatcher_ids<I>(mut self, dispatcher_ids: I) -> Self
    where
        I: IntoIterator<Item = DispatcherId>,
    {
        self.filter.dispatcher_ids = Some(dispatcher_ids.into_iter().collect());
        self
    }

    pub fn timestamp_after(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.timestamp_after = Some(ts);
        self
    }

    pub fn timestamp_before(mut self, ts: jiff::Timestamp) -> Self {
        self.filter.timestamp_before = Some(ts);
        self
    }

    pub fn battery_range(mut self, range: RangeInclusive<u8>) -> Self {
        self.filter.battery_range = Some(range);
        self
    }

    pub fn has_errors(mut self, has_errors: bool) -> Self {
        self.filter.has_errors = Some(has_errors);
        self
    }

    pub fn error_codes<I>(mut self, error_codes: I) -> Self
    where
        I: IntoIterator<Item = DeviceErrorCode>,
    {
        self.filter.error_codes = Some(error_codes.into_iter().collect());
        self
    }

    pub fn build(self) -> DeviceStatusFilter {
        self.filter
    }
}
