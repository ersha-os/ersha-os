use std::str::FromStr;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use ersha_core::{
    Device, DeviceId, DeviceKind, DeviceState, H3Cell, Percentage, Sensor, SensorId, SensorKind,
    SensorMetric,
};
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::ClickHouseError;
use crate::registry::{
    DeviceRegistry,
    filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder},
};

const CREATE_DEVICE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS devices (
    id String,
    kind Int32,
    state Int32,
    location Int64,
    manufacturer Nullable(String),
    provisioned_at Int64,
    sensor_count Int64,
    version UInt64
) ENGINE = ReplacingMergeTree(version)
ORDER BY id
"#;

const CREATE_SENSOR_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS sensors (
    id String,
    kind Int32,
    metric_type Int32,
    metric_value Float64,
    device_id String,
    version UInt64
) ENGINE = ReplacingMergeTree(version)
ORDER BY (device_id, id)
"#;

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct DeviceRow {
    id: String,
    kind: i32,
    state: i32,
    location: i64,
    manufacturer: Option<String>,
    provisioned_at: i64,
    sensor_count: i64,
    version: u64,
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct SensorRow {
    id: String,
    kind: i32,
    metric_type: i32,
    metric_value: f64,
    device_id: String,
    version: u64,
}

fn disect_metric(metric: &SensorMetric) -> (i32, f64) {
    match metric {
        SensorMetric::SoilMoisture { value } => (0, value.0 as f64),
        SensorMetric::SoilTemp { value } => (1, value.into_inner()),
        SensorMetric::AirTemp { value } => (2, value.into_inner()),
        SensorMetric::Humidity { value } => (3, value.0 as f64),
        SensorMetric::Rainfall { value } => (4, value.into_inner()),
    }
}

fn map_sensor_row(row: SensorRow) -> Result<Sensor, ClickHouseError> {
    let id = Ulid::from_str(&row.id).map_err(|_| ClickHouseError::InvalidUlid(row.id.clone()))?;

    let kind = match row.kind {
        0 => SensorKind::SoilMoisture,
        1 => SensorKind::SoilTemp,
        2 => SensorKind::AirTemp,
        3 => SensorKind::Humidity,
        4 => SensorKind::Rainfall,
        other => return Err(ClickHouseError::InvalidSensorKind(other)),
    };

    let metric = match row.metric_type {
        0 => SensorMetric::SoilMoisture {
            value: Percentage(row.metric_value as u8),
        },
        1 => SensorMetric::SoilTemp {
            value: NotNan::new(row.metric_value).expect("database should not contain NaN"),
        },
        2 => SensorMetric::AirTemp {
            value: NotNan::new(row.metric_value).expect("database should not contain NaN"),
        },
        3 => SensorMetric::Humidity {
            value: Percentage(row.metric_value as u8),
        },
        4 => SensorMetric::Rainfall {
            value: NotNan::new(row.metric_value).expect("database should not contain NaN"),
        },
        other => return Err(ClickHouseError::InvalidMetricType(other)),
    };

    Ok(Sensor {
        id: SensorId(id),
        kind,
        metric,
    })
}

#[derive(Clone)]
pub struct ClickHouseDeviceRegistry {
    client: Client,
}

impl ClickHouseDeviceRegistry {
    pub async fn new(url: &str, database: &str) -> Result<Self, ClickHouseError> {
        let client = super::create_client(url, database);
        client.query(CREATE_DEVICE_TABLE).execute().await?;
        client.query(CREATE_SENSOR_TABLE).execute().await?;
        Ok(Self { client })
    }

    async fn store_sensors(
        &self,
        device_id: DeviceId,
        sensors: impl Iterator<Item = Sensor> + Send,
    ) -> Result<(), ClickHouseError> {
        let version = jiff::Timestamp::now().as_millisecond() as u64;
        let mut insert = self.client.insert("sensors")?;
        let mut has_sensors = false;

        for sensor in sensors {
            has_sensors = true;
            let (metric_type, metric_value) = disect_metric(&sensor.metric);
            let row = SensorRow {
                id: sensor.id.0.to_string(),
                kind: sensor.kind as i32,
                metric_type,
                metric_value,
                device_id: device_id.0.to_string(),
                version,
            };
            insert.write(&row).await?;
        }

        if has_sensors {
            insert.end().await?;
        }
        Ok(())
    }

    async fn fetch_sensors(&self, device_id: DeviceId) -> Result<Box<[Sensor]>, ClickHouseError> {
        let rows: Vec<SensorRow> = self
            .client
            .query("SELECT ?fields FROM sensors FINAL WHERE device_id = ?")
            .bind(device_id.0.to_string())
            .fetch_all()
            .await?;

        let sensors: Result<Vec<_>, _> = rows.into_iter().map(map_sensor_row).collect();
        Ok(sensors?.into_boxed_slice())
    }

    fn map_device_row(&self, row: DeviceRow) -> Result<Device, ClickHouseError> {
        let id =
            Ulid::from_str(&row.id).map_err(|_| ClickHouseError::InvalidUlid(row.id.clone()))?;

        let kind = match row.kind {
            0 => DeviceKind::Sensor,
            other => return Err(ClickHouseError::InvalidDeviceKind(other)),
        };

        let state = match row.state {
            0 => DeviceState::Active,
            1 => DeviceState::Suspended,
            other => return Err(ClickHouseError::InvalidDeviceState(other)),
        };

        let provisioned_at = jiff::Timestamp::from_second(row.provisioned_at)
            .map_err(|_| ClickHouseError::InvalidTimestamp(row.provisioned_at))?;

        Ok(Device {
            id: DeviceId(id),
            kind,
            state,
            location: H3Cell(row.location as u64),
            manufacturer: row.manufacturer.map(|s| s.into_boxed_str()),
            provisioned_at,
            sensors: vec![].into_boxed_slice(),
        })
    }
}

#[async_trait]
impl DeviceRegistry for ClickHouseDeviceRegistry {
    type Error = ClickHouseError;

    async fn register(&self, device: Device) -> Result<(), Self::Error> {
        let version = jiff::Timestamp::now().as_millisecond() as u64;
        let row = DeviceRow {
            id: device.id.0.to_string(),
            kind: device.kind as i32,
            state: device.state as i32,
            location: device.location.0 as i64,
            manufacturer: device.manufacturer.as_ref().map(|s| s.to_string()),
            provisioned_at: device.provisioned_at.as_second(),
            sensor_count: device.sensors.len() as i64,
            version,
        };

        let mut insert = self.client.insert("devices")?;
        insert.write(&row).await?;
        insert.end().await?;

        self.store_sensors(device.id, device.sensors.into_vec().into_iter())
            .await?;

        Ok(())
    }

    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error> {
        let row: Option<DeviceRow> = self
            .client
            .query("SELECT ?fields FROM devices FINAL WHERE id = ?")
            .bind(id.0.to_string())
            .fetch_optional()
            .await?;

        match row {
            Some(r) => {
                let mut device = self.map_device_row(r)?;
                device.sensors = self.fetch_sensors(id).await?;
                Ok(Some(device))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, id: DeviceId, new: Device) -> Result<(), Self::Error> {
        let old = self.get(id).await?.ok_or(ClickHouseError::NotFound)?;
        let new = Device { id: old.id, ..new };
        self.register(new).await
    }

    async fn suspend(&self, id: DeviceId) -> Result<(), Self::Error> {
        let device = self.get(id).await?.ok_or(ClickHouseError::NotFound)?;
        let new = Device {
            state: DeviceState::Suspended,
            ..device
        };
        self.register(new).await
    }

    async fn add_sensor(&self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error> {
        self.store_sensors(id, std::iter::once(sensor)).await
    }

    async fn add_sensors(
        &self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor> + Send,
    ) -> Result<(), Self::Error> {
        self.store_sensors(id, sensors).await
    }

    async fn batch_register(&self, devices: Vec<Device>) -> Result<(), Self::Error> {
        if devices.is_empty() {
            return Ok(());
        }

        let version = jiff::Timestamp::now().as_millisecond() as u64;
        let mut insert = self.client.insert("devices")?;

        for device in &devices {
            let row = DeviceRow {
                id: device.id.0.to_string(),
                kind: device.kind.clone() as i32,
                state: device.state.clone() as i32,
                location: device.location.0 as i64,
                manufacturer: device.manufacturer.as_ref().map(|s| s.to_string()),
                provisioned_at: device.provisioned_at.as_second(),
                sensor_count: device.sensors.len() as i64,
                version,
            };
            insert.write(&row).await?;
        }
        insert.end().await?;

        for device in devices {
            self.store_sensors(device.id, device.sensors.into_vec().into_iter())
                .await?;
        }

        Ok(())
    }

    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error> {
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
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error> {
        let (query_str, bindings) = build_list_query(&options);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let rows: Vec<DeviceRow> = query.fetch_all().await?;

        if rows.is_empty() {
            return Ok(vec![]);
        }

        let mut devices = Vec::with_capacity(rows.len());
        for row in rows {
            let mut device = self.map_device_row(row)?;
            device.sensors = self.fetch_sensors(device.id).await?;
            devices.push(device);
        }

        Ok(devices)
    }
}

fn build_count_query(filter: Option<DeviceFilter>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT count() FROM devices FINAL");
    let mut bindings = Vec::new();

    if let Some(filter) = filter {
        let (where_clause, filter_bindings) = build_where_clause(&filter);
        if !where_clause.is_empty() {
            query.push_str(&where_clause);
            bindings = filter_bindings;
        }
    }

    (query, bindings)
}

fn build_list_query(options: &QueryOptions<DeviceFilter, DeviceSortBy>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT ?fields FROM devices FINAL");
    let (where_clause, bindings) = build_where_clause(&options.filter);

    if !where_clause.is_empty() {
        query.push_str(&where_clause);
    }

    query.push_str(" ORDER BY ");
    query.push_str(match options.sort_by {
        DeviceSortBy::State => "state",
        DeviceSortBy::Manufacturer => "manufacturer",
        DeviceSortBy::ProvisionAt => "provisioned_at",
        DeviceSortBy::SensorCount => "sensor_count",
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

fn build_where_clause(filter: &DeviceFilter) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();

    if let Some(ids) = &filter.ids
        && !ids.is_empty()
    {
        let placeholders: Vec<_> = ids.iter().map(|_| "?").collect();
        conditions.push(format!("id IN ({})", placeholders.join(", ")));
        bindings.extend(ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(states) = &filter.states
        && !states.is_empty()
    {
        let values: Vec<_> = states
            .iter()
            .map(|s| (s.clone() as i32).to_string())
            .collect();
        conditions.push(format!("state IN ({})", values.join(", ")));
    }

    if let Some(kinds) = &filter.kinds
        && !kinds.is_empty()
    {
        let values: Vec<_> = kinds
            .iter()
            .map(|k| (k.clone() as i32).to_string())
            .collect();
        conditions.push(format!("kind IN ({})", values.join(", ")));
    }

    if let Some(locations) = &filter.locations
        && !locations.is_empty()
    {
        let values: Vec<_> = locations.iter().map(|l| (l.0 as i64).to_string()).collect();
        conditions.push(format!("location IN ({})", values.join(", ")));
    }

    if let Some(after) = filter.provisioned_after {
        conditions.push(format!("provisioned_at >= {}", after.as_second()));
    }

    if let Some(before) = filter.provisioned_before {
        conditions.push(format!("provisioned_at <= {}", before.as_second()));
    }

    if let Some(range) = &filter.sensor_count {
        conditions.push(format!(
            "sensor_count BETWEEN {} AND {}",
            *range.start() as i64,
            *range.end() as i64
        ));
    }

    if let Some(pattern) = &filter.manufacturer_pattern {
        conditions.push(format!("manufacturer LIKE '%{}%'", pattern));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
}
