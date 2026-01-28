use std::str::FromStr;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use ersha_core::{
    DeviceError, DeviceErrorCode, DeviceId, DeviceStatus, DispatcherId, Percentage, SensorId,
    SensorState, SensorStatus, StatusId,
};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::ClickHouseError;
use crate::registry::{
    DeviceStatusRegistry,
    filter::{DeviceStatusFilter, DeviceStatusSortBy, Pagination, QueryOptions, SortOrder},
};

const CREATE_STATUS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS device_statuses (
    id String,
    device_id String,
    dispatcher_id String,
    battery_percent Int32,
    uptime_seconds Int64,
    signal_rssi Int32,
    timestamp Int64
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(toDateTime(timestamp))
ORDER BY (device_id, timestamp)
"#;

const CREATE_ERRORS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS device_status_errors (
    status_id String,
    error_code Int32,
    message Nullable(String)
) ENGINE = MergeTree()
ORDER BY status_id
"#;

const CREATE_SENSOR_STATUSES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS device_status_sensor_statuses (
    status_id String,
    sensor_id String,
    state Int32,
    last_reading Nullable(Int64)
) ENGINE = MergeTree()
ORDER BY status_id
"#;

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct StatusRow {
    id: String,
    device_id: String,
    dispatcher_id: String,
    battery_percent: i32,
    uptime_seconds: i64,
    signal_rssi: i32,
    timestamp: i64,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct ErrorRow {
    status_id: String,
    error_code: i32,
    message: Option<String>,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct SensorStatusRow {
    status_id: String,
    sensor_id: String,
    state: i32,
    last_reading: Option<i64>,
}

#[derive(Clone)]
pub struct ClickHouseDeviceStatusRegistry {
    client: Client,
}

impl ClickHouseDeviceStatusRegistry {
    pub async fn new(url: &str, database: &str) -> Result<Self, ClickHouseError> {
        let client = super::create_client(url, database);
        client.query(CREATE_STATUS_TABLE).execute().await?;
        client.query(CREATE_ERRORS_TABLE).execute().await?;
        client.query(CREATE_SENSOR_STATUSES_TABLE).execute().await?;
        Ok(Self { client })
    }

    async fn store_errors(
        &self,
        status_id: &StatusId,
        errors: &[DeviceError],
    ) -> Result<(), ClickHouseError> {
        if errors.is_empty() {
            return Ok(());
        }

        let mut insert = self.client.insert("device_status_errors")?;
        for error in errors {
            let error_code = match error.code {
                DeviceErrorCode::LowBattery => 0,
                DeviceErrorCode::SensorFault => 1,
                DeviceErrorCode::RadioFault => 2,
                DeviceErrorCode::Unknown => 3,
            };

            let row = ErrorRow {
                status_id: status_id.0.to_string(),
                error_code,
                message: error.message.as_ref().map(|s| s.to_string()),
            };
            insert.write(&row).await?;
        }
        insert.end().await?;
        Ok(())
    }

    async fn store_sensor_statuses(
        &self,
        status_id: &StatusId,
        sensor_statuses: &[SensorStatus],
    ) -> Result<(), ClickHouseError> {
        if sensor_statuses.is_empty() {
            return Ok(());
        }

        let mut insert = self.client.insert("device_status_sensor_statuses")?;
        for ss in sensor_statuses {
            let state = match ss.state {
                SensorState::Active => 0,
                SensorState::Faulty => 1,
                SensorState::Inactive => 2,
            };

            let row = SensorStatusRow {
                status_id: status_id.0.to_string(),
                sensor_id: ss.sensor_id.0.to_string(),
                state,
                last_reading: ss.last_reading.map(|t| t.as_second()),
            };
            insert.write(&row).await?;
        }
        insert.end().await?;
        Ok(())
    }

    async fn fetch_errors(
        &self,
        status_id: &StatusId,
    ) -> Result<Box<[DeviceError]>, ClickHouseError> {
        let rows: Vec<ErrorRow> = self
            .client
            .query("SELECT ?fields FROM device_status_errors WHERE status_id = ?")
            .bind(status_id.0.to_string())
            .fetch_all()
            .await?;

        let mut errors = Vec::with_capacity(rows.len());
        for row in rows {
            let code = match row.error_code {
                0 => DeviceErrorCode::LowBattery,
                1 => DeviceErrorCode::SensorFault,
                2 => DeviceErrorCode::RadioFault,
                3 => DeviceErrorCode::Unknown,
                other => return Err(ClickHouseError::InvalidErrorCode(other)),
            };

            errors.push(DeviceError {
                code,
                message: row.message.map(|s| s.into_boxed_str()),
            });
        }

        Ok(errors.into_boxed_slice())
    }

    async fn fetch_sensor_statuses(
        &self,
        status_id: &StatusId,
    ) -> Result<Box<[SensorStatus]>, ClickHouseError> {
        let rows: Vec<SensorStatusRow> = self
            .client
            .query("SELECT ?fields FROM device_status_sensor_statuses WHERE status_id = ?")
            .bind(status_id.0.to_string())
            .fetch_all()
            .await?;

        let mut sensor_statuses = Vec::with_capacity(rows.len());
        for row in rows {
            let sensor_id = Ulid::from_str(&row.sensor_id)
                .map_err(|_| ClickHouseError::InvalidUlid(row.sensor_id.clone()))?;

            let state = match row.state {
                0 => SensorState::Active,
                1 => SensorState::Faulty,
                2 => SensorState::Inactive,
                other => return Err(ClickHouseError::InvalidSensorState(other)),
            };

            let last_reading = row
                .last_reading
                .map(|ts| {
                    jiff::Timestamp::from_second(ts)
                        .map_err(|_| ClickHouseError::InvalidTimestamp(ts))
                })
                .transpose()?;

            sensor_statuses.push(SensorStatus {
                sensor_id: SensorId(sensor_id),
                state,
                last_reading,
            });
        }

        Ok(sensor_statuses.into_boxed_slice())
    }

    async fn map_row_to_status(&self, row: StatusRow) -> Result<DeviceStatus, ClickHouseError> {
        let id =
            Ulid::from_str(&row.id).map_err(|_| ClickHouseError::InvalidUlid(row.id.clone()))?;
        let device_id = Ulid::from_str(&row.device_id)
            .map_err(|_| ClickHouseError::InvalidUlid(row.device_id.clone()))?;
        let dispatcher_id = Ulid::from_str(&row.dispatcher_id)
            .map_err(|_| ClickHouseError::InvalidUlid(row.dispatcher_id.clone()))?;

        let timestamp = jiff::Timestamp::from_second(row.timestamp)
            .map_err(|_| ClickHouseError::InvalidTimestamp(row.timestamp))?;

        let status_id = StatusId(id);
        let errors = self.fetch_errors(&status_id).await?;
        let sensor_statuses = self.fetch_sensor_statuses(&status_id).await?;

        Ok(DeviceStatus {
            id: status_id,
            device_id: DeviceId(device_id),
            dispatcher_id: DispatcherId(dispatcher_id),
            battery_percent: Percentage(row.battery_percent as u8),
            uptime_seconds: row.uptime_seconds as u64,
            signal_rssi: row.signal_rssi as i16,
            errors,
            timestamp,
            sensor_statuses,
        })
    }
}

#[async_trait]
impl DeviceStatusRegistry for ClickHouseDeviceStatusRegistry {
    type Error = ClickHouseError;

    async fn store(&self, status: DeviceStatus) -> Result<(), Self::Error> {
        let row = StatusRow {
            id: status.id.0.to_string(),
            device_id: status.device_id.0.to_string(),
            dispatcher_id: status.dispatcher_id.0.to_string(),
            battery_percent: status.battery_percent.0 as i32,
            uptime_seconds: status.uptime_seconds as i64,
            signal_rssi: status.signal_rssi as i32,
            timestamp: status.timestamp.as_second(),
        };

        let mut insert = self.client.insert("device_statuses")?;
        insert.write(&row).await?;
        insert.end().await?;

        self.store_errors(&status.id, &status.errors).await?;
        self.store_sensor_statuses(&status.id, &status.sensor_statuses)
            .await?;

        Ok(())
    }

    async fn get(&self, id: StatusId) -> Result<Option<DeviceStatus>, Self::Error> {
        let row: Option<StatusRow> = self
            .client
            .query("SELECT ?fields FROM device_statuses WHERE id = ?")
            .bind(id.0.to_string())
            .fetch_optional()
            .await?;

        match row {
            Some(r) => Ok(Some(self.map_row_to_status(r).await?)),
            None => Ok(None),
        }
    }

    async fn get_latest(&self, device_id: DeviceId) -> Result<Option<DeviceStatus>, Self::Error> {
        let row: Option<StatusRow> = self
            .client
            .query("SELECT ?fields FROM device_statuses WHERE device_id = ? ORDER BY timestamp DESC LIMIT 1")
            .bind(device_id.0.to_string())
            .fetch_optional()
            .await?;

        match row {
            Some(r) => Ok(Some(self.map_row_to_status(r).await?)),
            None => Ok(None),
        }
    }

    async fn batch_store(&self, statuses: Vec<DeviceStatus>) -> Result<(), Self::Error> {
        for status in statuses {
            self.store(status).await?;
        }
        Ok(())
    }

    async fn count(&self, filter: Option<DeviceStatusFilter>) -> Result<usize, Self::Error> {
        let (query_str, bindings) = build_count_query(filter);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let count: u64 = query.fetch_one().await?;
        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<DeviceStatusFilter, DeviceStatusSortBy>,
    ) -> Result<Vec<DeviceStatus>, Self::Error> {
        let (query_str, bindings) = build_list_query(&options);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let rows: Vec<StatusRow> = query.fetch_all().await?;
        let mut statuses = Vec::with_capacity(rows.len());
        for row in rows {
            statuses.push(self.map_row_to_status(row).await?);
        }
        Ok(statuses)
    }
}

fn build_count_query(filter: Option<DeviceStatusFilter>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT count(DISTINCT id) FROM device_statuses");
    let mut bindings = Vec::new();

    if let Some(filter) = filter {
        let (join_clause, where_clause, filter_bindings) = build_clauses(&filter);
        query.push_str(&join_clause);
        if !where_clause.is_empty() {
            query.push_str(&where_clause);
            bindings = filter_bindings;
        }
    }

    (query, bindings)
}

fn build_list_query(
    options: &QueryOptions<DeviceStatusFilter, DeviceStatusSortBy>,
) -> (String, Vec<String>) {
    let mut query = String::from(
        "SELECT DISTINCT device_statuses.id, device_id, dispatcher_id, battery_percent, uptime_seconds, signal_rssi, timestamp FROM device_statuses",
    );
    let (join_clause, where_clause, bindings) = build_clauses(&options.filter);

    query.push_str(&join_clause);
    if !where_clause.is_empty() {
        query.push_str(&where_clause);
    }

    query.push_str(" ORDER BY ");
    query.push_str(match options.sort_by {
        DeviceStatusSortBy::Timestamp => "timestamp",
        DeviceStatusSortBy::BatteryPercent => "battery_percent",
        DeviceStatusSortBy::DeviceId => "device_id",
    });

    query.push_str(match options.sort_order {
        SortOrder::Asc => " ASC",
        SortOrder::Desc => " DESC",
    });

    match options.pagination {
        Pagination::Offset { offset, limit } => {
            query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
        }
        Pagination::Cursor { limit, after: _ } => {
            query.push_str(&format!(" LIMIT {}", limit));
        }
    }

    (query, bindings)
}

fn build_clauses(filter: &DeviceStatusFilter) -> (String, String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();
    let mut join_clause = String::new();

    let needs_error_join = filter.has_errors.is_some()
        || filter
            .error_codes
            .as_ref()
            .map(|v| !v.is_empty())
            .unwrap_or(false);

    if needs_error_join {
        join_clause = " LEFT JOIN device_status_errors ON device_statuses.id = device_status_errors.status_id".to_string();
    }

    if let Some(ids) = &filter.ids
        && !ids.is_empty()
    {
        let placeholders: Vec<_> = ids.iter().map(|_| "?").collect();
        conditions.push(format!(
            "device_statuses.id IN ({})",
            placeholders.join(", ")
        ));
        bindings.extend(ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(device_ids) = &filter.device_ids
        && !device_ids.is_empty()
    {
        let placeholders: Vec<_> = device_ids.iter().map(|_| "?").collect();
        conditions.push(format!("device_id IN ({})", placeholders.join(", ")));
        bindings.extend(device_ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(dispatcher_ids) = &filter.dispatcher_ids
        && !dispatcher_ids.is_empty()
    {
        let placeholders: Vec<_> = dispatcher_ids.iter().map(|_| "?").collect();
        conditions.push(format!("dispatcher_id IN ({})", placeholders.join(", ")));
        bindings.extend(dispatcher_ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(after) = filter.timestamp_after {
        conditions.push(format!("timestamp >= {}", after.as_second()));
    }

    if let Some(before) = filter.timestamp_before {
        conditions.push(format!("timestamp <= {}", before.as_second()));
    }

    if let Some(range) = &filter.battery_range {
        conditions.push(format!(
            "battery_percent BETWEEN {} AND {}",
            *range.start() as i32,
            *range.end() as i32
        ));
    }

    if let Some(has_errors) = filter.has_errors {
        if has_errors {
            conditions.push("device_status_errors.status_id IS NOT NULL".to_string());
        } else {
            conditions.push("device_status_errors.status_id IS NULL".to_string());
        }
    }

    if let Some(error_codes) = &filter.error_codes
        && !error_codes.is_empty()
    {
        let values: Vec<_> = error_codes
            .iter()
            .map(|code| {
                match code {
                    DeviceErrorCode::LowBattery => 0,
                    DeviceErrorCode::SensorFault => 1,
                    DeviceErrorCode::RadioFault => 2,
                    DeviceErrorCode::Unknown => 3,
                }
                .to_string()
            })
            .collect();
        conditions.push(format!(
            "device_status_errors.error_code IN ({})",
            values.join(", ")
        ));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (join_clause, where_clause, bindings)
}
