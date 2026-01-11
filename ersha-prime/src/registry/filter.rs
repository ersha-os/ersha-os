use ersha_core::{DeviceId, DeviceKind, DeviceState, DispatcherState, H3Cell};

use jiff;
use std::ops::RangeInclusive;
use ulid::Ulid;

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
