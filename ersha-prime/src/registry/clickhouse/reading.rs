use std::str::FromStr;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use ersha_core::{
    DeviceId, DispatcherId, H3Cell, Percentage, ReadingId, SensorId, SensorMetric, SensorReading,
};
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::ClickHouseError;
use crate::registry::{
    ReadingRegistry,
    filter::{Pagination, QueryOptions, ReadingFilter, ReadingSortBy, SensorMetricType, SortOrder},
};

const CREATE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS sensor_readings (
    id String,
    device_id String,
    dispatcher_id String,
    sensor_id String,
    metric_type Int32,
    metric_value Float64,
    location Int64,
    confidence Int32,
    timestamp Int64
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(toDateTime(timestamp))
ORDER BY (device_id, sensor_id, timestamp)
"#;

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct ReadingRow {
    id: String,
    device_id: String,
    dispatcher_id: String,
    sensor_id: String,
    metric_type: i32,
    metric_value: f64,
    location: i64,
    confidence: i32,
    timestamp: i64,
}

impl TryFrom<ReadingRow> for SensorReading {
    type Error = ClickHouseError;

    fn try_from(row: ReadingRow) -> Result<Self, Self::Error> {
        let id =
            Ulid::from_str(&row.id).map_err(|_| ClickHouseError::InvalidUlid(row.id.clone()))?;
        let device_id = Ulid::from_str(&row.device_id)
            .map_err(|_| ClickHouseError::InvalidUlid(row.device_id.clone()))?;
        let dispatcher_id = Ulid::from_str(&row.dispatcher_id)
            .map_err(|_| ClickHouseError::InvalidUlid(row.dispatcher_id.clone()))?;
        let sensor_id = Ulid::from_str(&row.sensor_id)
            .map_err(|_| ClickHouseError::InvalidUlid(row.sensor_id.clone()))?;

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

        let timestamp = jiff::Timestamp::from_second(row.timestamp)
            .map_err(|_| ClickHouseError::InvalidTimestamp(row.timestamp))?;

        Ok(SensorReading {
            id: ReadingId(id),
            device_id: DeviceId(device_id),
            dispatcher_id: DispatcherId(dispatcher_id),
            sensor_id: SensorId(sensor_id),
            metric,
            location: H3Cell(row.location as u64),
            confidence: Percentage(row.confidence as u8),
            timestamp,
        })
    }
}

impl From<&SensorReading> for ReadingRow {
    fn from(reading: &SensorReading) -> Self {
        let (metric_type, metric_value) = disect_metric(&reading.metric);
        ReadingRow {
            id: reading.id.0.to_string(),
            device_id: reading.device_id.0.to_string(),
            dispatcher_id: reading.dispatcher_id.0.to_string(),
            sensor_id: reading.sensor_id.0.to_string(),
            metric_type,
            metric_value,
            location: reading.location.0 as i64,
            confidence: reading.confidence.0 as i32,
            timestamp: reading.timestamp.as_second(),
        }
    }
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

#[derive(Clone)]
pub struct ClickHouseReadingRegistry {
    client: Client,
}

impl ClickHouseReadingRegistry {
    pub async fn new(url: &str, database: &str) -> Result<Self, ClickHouseError> {
        let client = super::create_client(url, database);
        client.query(CREATE_TABLE).execute().await?;
        Ok(Self { client })
    }
}

#[async_trait]
impl ReadingRegistry for ClickHouseReadingRegistry {
    type Error = ClickHouseError;

    async fn store(&self, reading: SensorReading) -> Result<(), Self::Error> {
        let row = ReadingRow::from(&reading);
        let mut insert = self.client.insert("sensor_readings")?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }

    async fn get(&self, id: ReadingId) -> Result<Option<SensorReading>, Self::Error> {
        let query = self
            .client
            .query("SELECT ?fields FROM sensor_readings WHERE id = ?")
            .bind(id.0.to_string());

        let row: Option<ReadingRow> = query.fetch_optional().await?;
        row.map(SensorReading::try_from).transpose()
    }

    async fn batch_store(&self, readings: Vec<SensorReading>) -> Result<(), Self::Error> {
        if readings.is_empty() {
            return Ok(());
        }

        let mut insert = self.client.insert("sensor_readings")?;
        for reading in &readings {
            let row = ReadingRow::from(reading);
            insert.write(&row).await?;
        }
        insert.end().await?;
        Ok(())
    }

    async fn count(&self, filter: Option<ReadingFilter>) -> Result<usize, Self::Error> {
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
        options: QueryOptions<ReadingFilter, ReadingSortBy>,
    ) -> Result<Vec<SensorReading>, Self::Error> {
        let (query_str, bindings) = build_list_query(&options);
        let mut query = self.client.query(&query_str);

        for binding in bindings {
            query = query.bind(binding);
        }

        let rows: Vec<ReadingRow> = query.fetch_all().await?;
        rows.into_iter().map(SensorReading::try_from).collect()
    }
}

fn build_count_query(filter: Option<ReadingFilter>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT count() FROM sensor_readings");
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

fn build_list_query(options: &QueryOptions<ReadingFilter, ReadingSortBy>) -> (String, Vec<String>) {
    let mut query = String::from("SELECT ?fields FROM sensor_readings");
    let (where_clause, bindings) = build_where_clause(&options.filter);

    if !where_clause.is_empty() {
        query.push_str(&where_clause);
    }

    query.push_str(" ORDER BY ");
    query.push_str(match options.sort_by {
        ReadingSortBy::Timestamp => "timestamp",
        ReadingSortBy::Confidence => "confidence",
        ReadingSortBy::DeviceId => "device_id",
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

fn build_where_clause(filter: &ReadingFilter) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();

    if let Some(ids) = &filter.ids
        && !ids.is_empty()
    {
        let placeholders: Vec<_> = ids.iter().map(|_| "?").collect();
        conditions.push(format!("id IN ({})", placeholders.join(", ")));
        bindings.extend(ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(device_ids) = &filter.device_ids
        && !device_ids.is_empty()
    {
        let placeholders: Vec<_> = device_ids.iter().map(|_| "?").collect();
        conditions.push(format!("device_id IN ({})", placeholders.join(", ")));
        bindings.extend(device_ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(sensor_ids) = &filter.sensor_ids
        && !sensor_ids.is_empty()
    {
        let placeholders: Vec<_> = sensor_ids.iter().map(|_| "?").collect();
        conditions.push(format!("sensor_id IN ({})", placeholders.join(", ")));
        bindings.extend(sensor_ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(dispatcher_ids) = &filter.dispatcher_ids
        && !dispatcher_ids.is_empty()
    {
        let placeholders: Vec<_> = dispatcher_ids.iter().map(|_| "?").collect();
        conditions.push(format!("dispatcher_id IN ({})", placeholders.join(", ")));
        bindings.extend(dispatcher_ids.iter().map(|id| id.0.to_string()));
    }

    if let Some(metric_types) = &filter.metric_types
        && !metric_types.is_empty()
    {
        let values: Vec<_> = metric_types
            .iter()
            .map(|mt| {
                match mt {
                    SensorMetricType::SoilMoisture => 0,
                    SensorMetricType::SoilTemp => 1,
                    SensorMetricType::AirTemp => 2,
                    SensorMetricType::Humidity => 3,
                    SensorMetricType::Rainfall => 4,
                }
                .to_string()
            })
            .collect();
        conditions.push(format!("metric_type IN ({})", values.join(", ")));
    }

    if let Some(locations) = &filter.locations
        && !locations.is_empty()
    {
        let values: Vec<_> = locations
            .iter()
            .map(|loc| (loc.0 as i64).to_string())
            .collect();
        conditions.push(format!("location IN ({})", values.join(", ")));
    }

    if let Some(after) = filter.timestamp_after {
        conditions.push(format!("timestamp >= {}", after.as_second()));
    }

    if let Some(before) = filter.timestamp_before {
        conditions.push(format!("timestamp <= {}", before.as_second()));
    }

    if let Some(range) = &filter.confidence_range {
        conditions.push(format!(
            "confidence BETWEEN {} AND {}",
            *range.start() as i32,
            *range.end() as i32
        ));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
}
