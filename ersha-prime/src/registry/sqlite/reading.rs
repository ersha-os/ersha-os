use std::str::FromStr;

use ersha_core::{
    DeviceId, DispatcherId, H3Cell, Percentage, ReadingId, SensorId, SensorMetric, SensorReading,
};
use ordered_float::NotNan;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions};
use ulid::Ulid;

use async_trait::async_trait;

use crate::registry::{
    ReadingRegistry,
    filter::{Pagination, QueryOptions, ReadingFilter, ReadingSortBy, SensorMetricType, SortOrder},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum SqliteReadingError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("invalid metric type: {0}")]
    InvalidMetricType(i32),
}

#[derive(Clone)]
pub struct SqliteReadingRegistry {
    pool: SqlitePool,
}

impl SqliteReadingRegistry {
    pub async fn new(path: impl AsRef<str>) -> Result<Self, SqliteReadingError> {
        let connection_string = format!("sqlite:{}", path.as_ref());
        let pool = SqlitePoolOptions::new().connect(&connection_string).await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn new_in_memory() -> Result<Self, SqliteReadingError> {
        let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl ReadingRegistry for SqliteReadingRegistry {
    type Error = SqliteReadingError;

    async fn store(&self, reading: SensorReading) -> Result<(), Self::Error> {
        let (metric_type, metric_value) = disect_metric(&reading.metric);

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO readings (id, device_id, dispatcher_id, sensor_id, metric_type, metric_value, location, confidence, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(reading.id.0.to_string())
        .bind(reading.device_id.0.to_string())
        .bind(reading.dispatcher_id.0.to_string())
        .bind(reading.sensor_id.0.to_string())
        .bind(metric_type)
        .bind(metric_value)
        .bind(reading.location.0 as i64)
        .bind(reading.confidence.0 as i32)
        .bind(reading.timestamp.as_second())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: ReadingId) -> Result<Option<SensorReading>, Self::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, device_id, dispatcher_id, sensor_id, metric_type, metric_value, location, confidence, timestamp
            FROM readings WHERE id = ?
            "#,
        )
        .bind(id.0.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| map_row_to_reading(&r)).transpose()
    }

    async fn batch_store(&self, readings: Vec<SensorReading>) -> Result<(), Self::Error> {
        let mut tx = self.pool.begin().await?;

        for reading in readings {
            let (metric_type, metric_value) = disect_metric(&reading.metric);

            sqlx::query(
                r#"
                INSERT OR REPLACE INTO readings (id, device_id, dispatcher_id, sensor_id, metric_type, metric_value, location, confidence, timestamp)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(reading.id.0.to_string())
            .bind(reading.device_id.0.to_string())
            .bind(reading.dispatcher_id.0.to_string())
            .bind(reading.sensor_id.0.to_string())
            .bind(metric_type)
            .bind(metric_value)
            .bind(reading.location.0 as i64)
            .bind(reading.confidence.0 as i32)
            .bind(reading.timestamp.as_second())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn count(&self, filter: Option<ReadingFilter>) -> Result<usize, Self::Error> {
        let mut query_builder = QueryBuilder::new("SELECT COUNT(*) FROM readings ");

        if let Some(filter) = filter {
            query_builder = filter_readings(query_builder, filter);
        }

        let query = query_builder.build();
        let count: i64 = query.fetch_one(&self.pool).await?.try_get(0)?;

        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<ReadingFilter, ReadingSortBy>,
    ) -> Result<Vec<SensorReading>, Self::Error> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, device_id, dispatcher_id, sensor_id, metric_type, metric_value, location, confidence, timestamp FROM readings ",
        );

        query_builder = filter_readings(query_builder, options.filter);

        query_builder.push(match options.sort_by {
            ReadingSortBy::Timestamp => " ORDER BY timestamp",
            ReadingSortBy::Confidence => " ORDER BY confidence",
            ReadingSortBy::DeviceId => " ORDER BY device_id",
        });

        query_builder.push(match options.sort_order {
            SortOrder::Asc => " ASC ",
            SortOrder::Desc => " DESC ",
        });

        match options.pagination {
            Pagination::Offset { offset, limit } => {
                query_builder.push(" LIMIT ").push_bind(limit as i64);
                query_builder.push(" OFFSET ").push_bind(offset as i64);
            }
            Pagination::Cursor { limit, after: _ } => {
                query_builder.push(" LIMIT ").push_bind(limit as i64);
            }
        }

        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        rows.iter().map(map_row_to_reading).collect()
    }
}

fn map_row_to_reading(r: &sqlx::sqlite::SqliteRow) -> Result<SensorReading, SqliteReadingError> {
    let id_str: String = r.try_get("id")?;
    let id = Ulid::from_str(&id_str).map_err(|_| SqliteReadingError::InvalidUlid(id_str))?;

    let device_id_str: String = r.try_get("device_id")?;
    let device_id = Ulid::from_str(&device_id_str)
        .map_err(|_| SqliteReadingError::InvalidUlid(device_id_str))?;

    let dispatcher_id_str: String = r.try_get("dispatcher_id")?;
    let dispatcher_id = Ulid::from_str(&dispatcher_id_str)
        .map_err(|_| SqliteReadingError::InvalidUlid(dispatcher_id_str))?;

    let sensor_id_str: String = r.try_get("sensor_id")?;
    let sensor_id = Ulid::from_str(&sensor_id_str)
        .map_err(|_| SqliteReadingError::InvalidUlid(sensor_id_str))?;

    let metric_type: i32 = r.try_get("metric_type")?;
    let metric_value: f64 = r.try_get("metric_value")?;

    let metric = match metric_type {
        0 => SensorMetric::SoilMoisture {
            value: Percentage(metric_value as u8),
        },
        1 => SensorMetric::SoilTemp {
            value: NotNan::new(metric_value).expect("database should not contain NaN"),
        },
        2 => SensorMetric::AirTemp {
            value: NotNan::new(metric_value).expect("database should not contain NaN"),
        },
        3 => SensorMetric::Humidity {
            value: Percentage(metric_value as u8),
        },
        4 => SensorMetric::Rainfall {
            value: NotNan::new(metric_value).expect("database should not contain NaN"),
        },
        other => return Err(SqliteReadingError::InvalidMetricType(other)),
    };

    let timestamp_sec: i64 = r.try_get("timestamp")?;
    let timestamp = jiff::Timestamp::from_second(timestamp_sec)
        .map_err(|_| SqliteReadingError::InvalidTimestamp(timestamp_sec))?;

    Ok(SensorReading {
        id: ReadingId(id),
        device_id: DeviceId(device_id),
        dispatcher_id: DispatcherId(dispatcher_id),
        sensor_id: SensorId(sensor_id),
        metric,
        location: H3Cell(r.try_get::<i64, _>("location")? as u64),
        confidence: Percentage(r.try_get::<i32, _>("confidence")? as u8),
        timestamp,
    })
}

fn filter_readings<'a>(
    mut query_builder: QueryBuilder<'a, Sqlite>,
    filter: ReadingFilter,
) -> QueryBuilder<'a, Sqlite> {
    let mut has_where = false;

    let mut prefix = |qb: &mut QueryBuilder<'a, Sqlite>| {
        if has_where {
            qb.push(" AND ");
        } else {
            qb.push(" WHERE ");
            has_where = true;
        }
    };

    if let Some(ids) = filter.ids
        && !ids.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("id IN (");
        let mut separated = query_builder.separated(", ");
        for id in ids {
            separated.push_bind(id.0.to_string());
        }
        separated.push_unseparated(")");
    }

    if let Some(device_ids) = filter.device_ids
        && !device_ids.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("device_id IN (");
        let mut separated = query_builder.separated(", ");
        for id in device_ids {
            separated.push_bind(id.0.to_string());
        }
        separated.push_unseparated(")");
    }

    if let Some(sensor_ids) = filter.sensor_ids
        && !sensor_ids.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("sensor_id IN (");
        let mut separated = query_builder.separated(", ");
        for id in sensor_ids {
            separated.push_bind(id.0.to_string());
        }
        separated.push_unseparated(")");
    }

    if let Some(dispatcher_ids) = filter.dispatcher_ids
        && !dispatcher_ids.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("dispatcher_id IN (");
        let mut separated = query_builder.separated(", ");
        for id in dispatcher_ids {
            separated.push_bind(id.0.to_string());
        }
        separated.push_unseparated(")");
    }

    if let Some(metric_types) = filter.metric_types
        && !metric_types.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("metric_type IN (");
        let mut separated = query_builder.separated(", ");
        for mt in metric_types {
            let val = match mt {
                SensorMetricType::SoilMoisture => 0,
                SensorMetricType::SoilTemp => 1,
                SensorMetricType::AirTemp => 2,
                SensorMetricType::Humidity => 3,
                SensorMetricType::Rainfall => 4,
            };
            separated.push_bind(val);
        }
        separated.push_unseparated(")");
    }

    if let Some(locations) = filter.locations
        && !locations.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("location IN (");
        let mut separated = query_builder.separated(", ");
        for loc in locations {
            separated.push_bind(loc.0 as i64);
        }
        separated.push_unseparated(")");
    }

    if let Some(after) = filter.timestamp_after {
        prefix(&mut query_builder);
        query_builder
            .push("timestamp >= ")
            .push_bind(after.as_second());
    }

    if let Some(before) = filter.timestamp_before {
        prefix(&mut query_builder);
        query_builder
            .push("timestamp <= ")
            .push_bind(before.as_second());
    }

    if let Some(range) = filter.confidence_range {
        prefix(&mut query_builder);
        query_builder
            .push("confidence BETWEEN ")
            .push_bind(*range.start() as i32)
            .push(" AND ")
            .push_bind(*range.end() as i32);
    }

    query_builder
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

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use ordered_float::NotNan;
    use ulid::Ulid;

    use crate::registry::ReadingRegistry;
    use crate::registry::filter::{
        Pagination, QueryOptions, ReadingFilter, ReadingSortBy, SensorMetricType, SortOrder,
    };
    use ersha_core::{
        DeviceId, DispatcherId, H3Cell, Percentage, ReadingId, SensorId, SensorMetric,
        SensorReading,
    };

    use super::SqliteReadingRegistry;

    fn mock_reading(id: ReadingId, metric: SensorMetric, confidence: u8) -> SensorReading {
        SensorReading {
            id,
            device_id: DeviceId(Ulid::new()),
            dispatcher_id: DispatcherId(Ulid::new()),
            metric,
            location: H3Cell(0x8a2a1072b59ffff),
            confidence: Percentage(confidence),
            timestamp: Timestamp::now(),
            sensor_id: SensorId(Ulid::new()),
        }
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let registry = SqliteReadingRegistry::new_in_memory().await.unwrap();
        let id = ReadingId(Ulid::new());
        let reading = mock_reading(
            id,
            SensorMetric::AirTemp {
                value: NotNan::new(25.0).unwrap(),
            },
            90,
        );

        registry.store(reading.clone()).await.unwrap();

        let fetched = registry.get(id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.confidence.0, 90);
    }

    #[tokio::test]
    async fn test_batch_store_and_count() {
        let registry = SqliteReadingRegistry::new_in_memory().await.unwrap();

        let readings = vec![
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(20.0).unwrap(),
                },
                80,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::Humidity {
                    value: Percentage(60),
                },
                90,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(22.0).unwrap(),
                },
                95,
            ),
        ];

        registry.batch_store(readings).await.unwrap();

        assert_eq!(registry.count(None).await.unwrap(), 3);

        let filter = ReadingFilter {
            metric_types: Some(vec![SensorMetricType::AirTemp]),
            ..Default::default()
        };
        assert_eq!(registry.count(Some(filter)).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_list_with_sorting() {
        let registry = SqliteReadingRegistry::new_in_memory().await.unwrap();

        let readings = vec![
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(20.0).unwrap(),
                },
                80,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(22.0).unwrap(),
                },
                95,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(21.0).unwrap(),
                },
                70,
            ),
        ];

        registry.batch_store(readings).await.unwrap();

        let options = QueryOptions {
            filter: ReadingFilter::default(),
            sort_by: ReadingSortBy::Confidence,
            sort_order: SortOrder::Desc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].confidence.0, 95);
        assert_eq!(results[1].confidence.0, 80);
        assert_eq!(results[2].confidence.0, 70);
    }

    #[tokio::test]
    async fn test_filter_by_confidence_range() {
        let registry = SqliteReadingRegistry::new_in_memory().await.unwrap();

        let readings = vec![
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(20.0).unwrap(),
                },
                50,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(22.0).unwrap(),
                },
                75,
            ),
            mock_reading(
                ReadingId(Ulid::new()),
                SensorMetric::AirTemp {
                    value: NotNan::new(21.0).unwrap(),
                },
                90,
            ),
        ];

        registry.batch_store(readings).await.unwrap();

        let filter = ReadingFilter {
            confidence_range: Some(70..=80),
            ..Default::default()
        };

        assert_eq!(registry.count(Some(filter)).await.unwrap(), 1);
    }
}
