use std::str::FromStr;

use ersha_core::{
    DeviceError, DeviceErrorCode, DeviceId, DeviceStatus, DispatcherId, Percentage, SensorId,
    SensorState, SensorStatus, StatusId,
};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions};
use ulid::Ulid;

use async_trait::async_trait;

use crate::registry::{
    DeviceStatusRegistry,
    filter::{DeviceStatusFilter, DeviceStatusSortBy, Pagination, QueryOptions, SortOrder},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum SqliteDeviceStatusError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("invalid error code: {0}")]
    InvalidErrorCode(i32),
    #[error("invalid sensor state: {0}")]
    InvalidSensorState(i32),
}

#[derive(Clone)]
pub struct SqliteDeviceStatusRegistry {
    pool: SqlitePool,
}

impl SqliteDeviceStatusRegistry {
    pub async fn new(path: impl AsRef<str>) -> Result<Self, SqliteDeviceStatusError> {
        let connection_string = format!("sqlite:{}", path.as_ref());
        let pool = SqlitePoolOptions::new().connect(&connection_string).await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn new_in_memory() -> Result<Self, SqliteDeviceStatusError> {
        let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl DeviceStatusRegistry for SqliteDeviceStatusRegistry {
    type Error = SqliteDeviceStatusError;

    async fn store(&self, status: DeviceStatus) -> Result<(), Self::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO device_statuses (id, device_id, dispatcher_id, battery_percent, uptime_seconds, signal_rssi, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(status.id.0.to_string())
        .bind(status.device_id.0.to_string())
        .bind(status.dispatcher_id.0.to_string())
        .bind(status.battery_percent.0 as i32)
        .bind(status.uptime_seconds as i64)
        .bind(status.signal_rssi as i32)
        .bind(status.timestamp.as_second())
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM device_status_errors WHERE status_id = ?")
            .bind(status.id.0.to_string())
            .execute(&mut *tx)
            .await?;

        for error in status.errors.iter() {
            let error_code = match error.code {
                DeviceErrorCode::LowBattery => 0,
                DeviceErrorCode::SensorFault => 1,
                DeviceErrorCode::RadioFault => 2,
                DeviceErrorCode::Unknown => 3,
            };

            sqlx::query(
                r#"
                INSERT INTO device_status_errors (status_id, error_code, message)
                VALUES (?, ?, ?)
                "#,
            )
            .bind(status.id.0.to_string())
            .bind(error_code)
            .bind(error.message.as_deref())
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("DELETE FROM device_status_sensor_statuses WHERE status_id = ?")
            .bind(status.id.0.to_string())
            .execute(&mut *tx)
            .await?;

        for sensor_status in status.sensor_statuses.iter() {
            let state = match sensor_status.state {
                SensorState::Active => 0,
                SensorState::Faulty => 1,
                SensorState::Inactive => 2,
            };

            sqlx::query(
                r#"
                INSERT INTO device_status_sensor_statuses (status_id, sensor_id, state, last_reading)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(status.id.0.to_string())
            .bind(sensor_status.sensor_id.0.to_string())
            .bind(state)
            .bind(sensor_status.last_reading.map(|t| t.as_second()))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get(&self, id: StatusId) -> Result<Option<DeviceStatus>, Self::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, device_id, dispatcher_id, battery_percent, uptime_seconds, signal_rssi, timestamp
            FROM device_statuses WHERE id = ?
            "#,
        )
        .bind(id.0.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = row else {
            return Ok(None);
        };

        let status = self.map_row_to_status(&r).await?;
        Ok(Some(status))
    }

    async fn get_latest(&self, device_id: DeviceId) -> Result<Option<DeviceStatus>, Self::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, device_id, dispatcher_id, battery_percent, uptime_seconds, signal_rssi, timestamp
            FROM device_statuses WHERE device_id = ?
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(device_id.0.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = row else {
            return Ok(None);
        };

        let status = self.map_row_to_status(&r).await?;
        Ok(Some(status))
    }

    async fn batch_store(&self, statuses: Vec<DeviceStatus>) -> Result<(), Self::Error> {
        for status in statuses {
            self.store(status).await?;
        }
        Ok(())
    }

    async fn count(&self, filter: Option<DeviceStatusFilter>) -> Result<usize, Self::Error> {
        let mut query_builder =
            QueryBuilder::new("SELECT COUNT(DISTINCT device_statuses.id) FROM device_statuses ");

        if let Some(filter) = filter {
            query_builder = filter_statuses(query_builder, filter);
        }

        let query = query_builder.build();
        let count: i64 = query.fetch_one(&self.pool).await?.try_get(0)?;

        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<DeviceStatusFilter, DeviceStatusSortBy>,
    ) -> Result<Vec<DeviceStatus>, Self::Error> {
        let mut query_builder = QueryBuilder::new(
            "SELECT DISTINCT device_statuses.id, device_id, dispatcher_id, battery_percent, uptime_seconds, signal_rssi, timestamp FROM device_statuses ",
        );

        query_builder = filter_statuses(query_builder, options.filter);

        query_builder.push(match options.sort_by {
            DeviceStatusSortBy::Timestamp => " ORDER BY timestamp",
            DeviceStatusSortBy::BatteryPercent => " ORDER BY battery_percent",
            DeviceStatusSortBy::DeviceId => " ORDER BY device_id",
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

        let mut statuses = Vec::with_capacity(rows.len());
        for row in &rows {
            statuses.push(self.map_row_to_status(row).await?);
        }

        Ok(statuses)
    }
}

impl SqliteDeviceStatusRegistry {
    async fn map_row_to_status(
        &self,
        r: &sqlx::sqlite::SqliteRow,
    ) -> Result<DeviceStatus, SqliteDeviceStatusError> {
        let id_str: String = r.try_get("id")?;
        let id =
            Ulid::from_str(&id_str).map_err(|_| SqliteDeviceStatusError::InvalidUlid(id_str))?;

        let device_id_str: String = r.try_get("device_id")?;
        let device_id = Ulid::from_str(&device_id_str)
            .map_err(|_| SqliteDeviceStatusError::InvalidUlid(device_id_str))?;

        let dispatcher_id_str: String = r.try_get("dispatcher_id")?;
        let dispatcher_id = Ulid::from_str(&dispatcher_id_str)
            .map_err(|_| SqliteDeviceStatusError::InvalidUlid(dispatcher_id_str))?;

        let timestamp_sec: i64 = r.try_get("timestamp")?;
        let timestamp = jiff::Timestamp::from_second(timestamp_sec)
            .map_err(|_| SqliteDeviceStatusError::InvalidTimestamp(timestamp_sec))?;

        let errors = self.fetch_errors(StatusId(id)).await?;
        let sensor_statuses = self.fetch_sensor_statuses(StatusId(id)).await?;

        Ok(DeviceStatus {
            id: StatusId(id),
            device_id: DeviceId(device_id),
            dispatcher_id: DispatcherId(dispatcher_id),
            battery_percent: Percentage(r.try_get::<i32, _>("battery_percent")? as u8),
            uptime_seconds: r.try_get::<i64, _>("uptime_seconds")? as u64,
            signal_rssi: r.try_get::<i32, _>("signal_rssi")? as i16,
            errors,
            timestamp,
            sensor_statuses,
        })
    }

    async fn fetch_errors(
        &self,
        status_id: StatusId,
    ) -> Result<Box<[DeviceError]>, SqliteDeviceStatusError> {
        let rows = sqlx::query(
            r#"SELECT error_code, message FROM device_status_errors WHERE status_id = ?"#,
        )
        .bind(status_id.0.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut errors = Vec::with_capacity(rows.len());
        for row in rows {
            let code = match row.try_get::<i32, _>("error_code")? {
                0 => DeviceErrorCode::LowBattery,
                1 => DeviceErrorCode::SensorFault,
                2 => DeviceErrorCode::RadioFault,
                3 => DeviceErrorCode::Unknown,
                other => return Err(SqliteDeviceStatusError::InvalidErrorCode(other)),
            };

            let message: Option<String> = row.try_get("message")?;

            errors.push(DeviceError {
                code,
                message: message.map(|s| s.into_boxed_str()),
            });
        }

        Ok(errors.into_boxed_slice())
    }

    async fn fetch_sensor_statuses(
        &self,
        status_id: StatusId,
    ) -> Result<Box<[SensorStatus]>, SqliteDeviceStatusError> {
        let rows = sqlx::query(
            r#"SELECT sensor_id, state, last_reading FROM device_status_sensor_statuses WHERE status_id = ?"#,
        )
        .bind(status_id.0.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut sensor_statuses = Vec::with_capacity(rows.len());
        for row in rows {
            let sensor_id_str: String = row.try_get("sensor_id")?;
            let sensor_id = Ulid::from_str(&sensor_id_str)
                .map_err(|_| SqliteDeviceStatusError::InvalidUlid(sensor_id_str))?;

            let state = match row.try_get::<i32, _>("state")? {
                0 => SensorState::Active,
                1 => SensorState::Faulty,
                2 => SensorState::Inactive,
                other => return Err(SqliteDeviceStatusError::InvalidSensorState(other)),
            };

            let last_reading: Option<i64> = row.try_get("last_reading")?;
            let last_reading = last_reading
                .map(|ts| {
                    jiff::Timestamp::from_second(ts)
                        .map_err(|_| SqliteDeviceStatusError::InvalidTimestamp(ts))
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
}

fn filter_statuses<'a>(
    mut query_builder: QueryBuilder<'a, Sqlite>,
    filter: DeviceStatusFilter,
) -> QueryBuilder<'a, Sqlite> {
    let mut has_where = false;
    let mut needs_error_join = false;

    if filter.has_errors.is_some()
        || (filter.error_codes.is_some() && !filter.error_codes.as_ref().unwrap().is_empty())
    {
        needs_error_join = true;
    }

    if needs_error_join {
        query_builder.push(" LEFT JOIN device_status_errors ON device_statuses.id = device_status_errors.status_id ");
    }

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
        query_builder.push("device_statuses.id IN (");
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

    if let Some(range) = filter.battery_range {
        prefix(&mut query_builder);
        query_builder
            .push("battery_percent BETWEEN ")
            .push_bind(*range.start() as i32)
            .push(" AND ")
            .push_bind(*range.end() as i32);
    }

    if let Some(has_errors) = filter.has_errors {
        prefix(&mut query_builder);
        if has_errors {
            query_builder.push("device_status_errors.id IS NOT NULL");
        } else {
            query_builder.push("device_status_errors.id IS NULL");
        }
    }

    if let Some(error_codes) = filter.error_codes
        && !error_codes.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("device_status_errors.error_code IN (");
        let mut separated = query_builder.separated(", ");
        for code in error_codes {
            let val = match code {
                DeviceErrorCode::LowBattery => 0,
                DeviceErrorCode::SensorFault => 1,
                DeviceErrorCode::RadioFault => 2,
                DeviceErrorCode::Unknown => 3,
            };
            separated.push_bind(val);
        }
        separated.push_unseparated(")");
    }

    query_builder
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use ulid::Ulid;

    use crate::registry::DeviceStatusRegistry;
    use crate::registry::filter::{
        DeviceStatusFilter, DeviceStatusSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{
        DeviceError, DeviceErrorCode, DeviceId, DeviceStatus, DispatcherId, Percentage, StatusId,
    };

    use super::SqliteDeviceStatusRegistry;

    fn mock_status(id: StatusId, device_id: DeviceId, battery: u8) -> DeviceStatus {
        DeviceStatus {
            id,
            device_id,
            dispatcher_id: DispatcherId(Ulid::new()),
            battery_percent: Percentage(battery),
            uptime_seconds: 3600,
            signal_rssi: -50,
            errors: vec![].into_boxed_slice(),
            timestamp: Timestamp::now(),
            sensor_statuses: vec![].into_boxed_slice(),
        }
    }

    fn mock_status_with_errors(
        id: StatusId,
        device_id: DeviceId,
        battery: u8,
        errors: Vec<DeviceError>,
    ) -> DeviceStatus {
        DeviceStatus {
            errors: errors.into_boxed_slice(),
            ..mock_status(id, device_id, battery)
        }
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();
        let id = StatusId(Ulid::new());
        let device_id = DeviceId(Ulid::new());
        let status = mock_status(id, device_id, 85);

        registry.store(status.clone()).await.unwrap();

        let fetched = registry.get(id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.battery_percent.0, 85);
    }

    #[tokio::test]
    async fn test_get_latest() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();
        let device_id = DeviceId(Ulid::new());

        let mut older = mock_status(StatusId(Ulid::new()), device_id, 90);
        older.timestamp = Timestamp::from_second(100).unwrap();

        let mut newer = mock_status(StatusId(Ulid::new()), device_id, 80);
        newer.timestamp = Timestamp::from_second(200).unwrap();

        registry
            .batch_store(vec![older, newer.clone()])
            .await
            .unwrap();

        let latest = registry.get_latest(device_id).await.unwrap().unwrap();
        assert_eq!(latest.battery_percent.0, 80);
    }

    #[tokio::test]
    async fn test_filter_by_battery_range() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();

        let statuses = vec![
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 20),
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 50),
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 80),
        ];

        registry.batch_store(statuses).await.unwrap();

        let filter = DeviceStatusFilter {
            battery_range: Some(40..=60),
            ..Default::default()
        };

        assert_eq!(registry.count(Some(filter)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_filter_by_has_errors() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();

        let statuses = vec![
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 90),
            mock_status_with_errors(
                StatusId(Ulid::new()),
                DeviceId(Ulid::new()),
                50,
                vec![DeviceError {
                    code: DeviceErrorCode::LowBattery,
                    message: None,
                }],
            ),
        ];

        registry.batch_store(statuses).await.unwrap();

        let filter_with_errors = DeviceStatusFilter {
            has_errors: Some(true),
            ..Default::default()
        };
        assert_eq!(registry.count(Some(filter_with_errors)).await.unwrap(), 1);

        let filter_no_errors = DeviceStatusFilter {
            has_errors: Some(false),
            ..Default::default()
        };
        assert_eq!(registry.count(Some(filter_no_errors)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_sorted_by_battery() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();

        let statuses = vec![
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 50),
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 90),
            mock_status(StatusId(Ulid::new()), DeviceId(Ulid::new()), 20),
        ];

        registry.batch_store(statuses).await.unwrap();

        let options = QueryOptions {
            filter: DeviceStatusFilter::default(),
            sort_by: DeviceStatusSortBy::BatteryPercent,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].battery_percent.0, 20);
        assert_eq!(results[1].battery_percent.0, 50);
        assert_eq!(results[2].battery_percent.0, 90);
    }

    #[tokio::test]
    async fn test_store_with_errors_and_sensor_statuses() {
        let registry = SqliteDeviceStatusRegistry::new_in_memory().await.unwrap();
        let id = StatusId(Ulid::new());
        let device_id = DeviceId(Ulid::new());

        let status = DeviceStatus {
            id,
            device_id,
            dispatcher_id: DispatcherId(Ulid::new()),
            battery_percent: Percentage(75),
            uptime_seconds: 7200,
            signal_rssi: -60,
            errors: vec![
                DeviceError {
                    code: DeviceErrorCode::LowBattery,
                    message: Some("Battery low".into()),
                },
                DeviceError {
                    code: DeviceErrorCode::SensorFault,
                    message: None,
                },
            ]
            .into_boxed_slice(),
            timestamp: Timestamp::now(),
            sensor_statuses: vec![ersha_core::SensorStatus {
                sensor_id: ersha_core::SensorId(Ulid::new()),
                state: ersha_core::SensorState::Active,
                last_reading: Some(Timestamp::now()),
            }]
            .into_boxed_slice(),
        };

        registry.store(status.clone()).await.unwrap();

        let fetched = registry.get(id).await.unwrap().unwrap();
        assert_eq!(fetched.errors.len(), 2);
        assert_eq!(fetched.sensor_statuses.len(), 1);
        assert_eq!(fetched.errors[0].code, DeviceErrorCode::LowBattery);
    }
}
