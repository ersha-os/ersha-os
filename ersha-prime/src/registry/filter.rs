#[allow(dead_code)]
use ersha_core::{DeviceId, DeviceKind, DeviceState, DispatcherState, H3Cell};

use jiff;
use std::ops::RangeInclusive;

pub enum SortBy {
    Id,
    ProvisionAt,
    Location,
    Manufacturer,
    State,
}

pub enum SortOrder {
    Asc,
    Desc,
}

pub enum Pagination {
    Offset {
        offset: usize,
        limit: usize,
    },
    Cursor {
        after: Option<DeviceId>,
        limit: usize,
    },
}

pub struct QueryOptions<F> {
    pub filter: F,
    pub sort_by: Option<SortBy>,
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

#[derive(Default)]
pub struct DispatcherFilter {
    pub states: Option<Vec<DispatcherState>>,
    pub locations: Option<Vec<H3Cell>>,
}
