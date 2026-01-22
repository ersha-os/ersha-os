use std::str::FromStr;

use ersha_core::{
    Device, DeviceId, DeviceKind, DeviceState, H3Cell, Percentage, Sensor, SensorId, SensorKind,
    SensorMetric,
};
use ordered_float::NotNan;
use sqlx::{
    QueryBuilder, Row, Sqlite, SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions,
    sqlite::SqliteRow,
};
use ulid::Ulid;

use async_trait::async_trait;

use crate::registry::{
    DeviceRegistry,
    filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum SqliteDeviceError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("invalid device state: {0}")]
    InvalidState(i32),
    #[error("invalid device kind: {0}")]
    InvalidDeviceKind(i32),
    #[error("invalid metric type: {0}")]
    InvalidMetricType(i32),
    #[error("invalid sensor kind: {0}")]
    InvalidSensorKind(i32),
    #[error("not found")]
    NotFound,
}

pub struct SqliteDeviceRegistry {
    pool: SqlitePool,
}

impl SqliteDeviceRegistry {
    pub async fn new(path: impl AsRef<str>) -> Result<Self, SqliteDeviceError> {
        let connection_string = format!("sqlite:{}", path.as_ref());
        let pool = SqlitePoolOptions::new().connect(&connection_string).await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn new_in_memory() -> Result<Self, SqliteDeviceError> {
        let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl DeviceRegistry for SqliteDeviceRegistry {
    type Error = SqliteDeviceError;

    async fn register(&self, device: Device) -> Result<(), Self::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO devices (id, kind, state, location, manufacturer, provisioned_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(device.id.0.to_string())
        .bind(device.kind as i32)
        .bind(device.state as i32)
        .bind(device.location.0 as i64)
        .bind(device.manufacturer)
        .bind(device.provisioned_at.as_second())
        .execute(&self.pool)
        .await?;

        self.add_sensors(device.id, device.sensors.into_iter())
            .await?;

        Ok(())
    }

    async fn add_sensor(&self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error> {
        let (metric_type, metric_value) = disect_metric(sensor.metric);

        sqlx::query(
            r#"
             INSERT OR REPLACE INTO sensors (id, kind, metric_type, metric_value, device_id)
             VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(sensor.id.0.to_string())
        .bind(sensor.kind as i32)
        .bind(metric_type)
        .bind(metric_value)
        .bind(id.0.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_sensors(
        &self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor> + Send,
    ) -> Result<(), Self::Error> {
        let mut tx = self.pool.begin().await?;

        for sensor in sensors {
            let (metric_type, metric_value) = disect_metric(sensor.metric);

            sqlx::query(
                r#"
             INSERT OR REPLACE INTO sensors (id, kind, metric_type, metric_value, device_id)
             VALUES (?, ?, ?, ?, ?)
            "#,
            )
            .bind(sensor.id.0.to_string())
            .bind(sensor.kind as i32)
            .bind(metric_type)
            .bind(metric_value)
            .bind(id.0.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get(&self, id: DeviceId) -> Result<Option<Device>, Self::Error> {
        let device_row = sqlx::query(
            r#"SELECT id, kind, state, location, manufacturer, provisioned_at FROM devices WHERE id = ?"#,
        )
        .bind(id.0.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = device_row else {
            return Ok(None);
        };

        let sensor_rows = sqlx::query(
            r#"SELECT id, kind, metric_type, metric_value FROM sensors WHERE device_id = ?"#,
        )
        .bind(id.0.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut sensors = Vec::with_capacity(sensor_rows.len());
        for s_row in sensor_rows {
            let s_id_str = s_row.try_get::<String, _>("id")?;
            let s_ulid =
                Ulid::from_str(&s_id_str).map_err(|_| Self::Error::InvalidUlid(s_id_str))?;

            sensors.push(Sensor {
                id: SensorId(s_ulid),
                kind: match s_row.try_get::<i32, _>("kind")? {
                    0 => SensorKind::SoilMoisture,
                    1 => SensorKind::SoilTemp,
                    2 => SensorKind::AirTemp,
                    3 => SensorKind::Humidity,
                    4 => SensorKind::Rainfall,
                    other => return Err(Self::Error::InvalidSensorKind(other)),
                },

                metric: match s_row.try_get::<i32, _>("metric_type")? {
                    0 => SensorMetric::SoilMoisture {
                        value: Percentage(s_row.try_get::<f64, _>("metric_value")? as u8),
                    },
                    1 => SensorMetric::SoilTemp {
                        value: NotNan::new(s_row.try_get::<f64, _>("metric_value")?)
                            .expect("database should not contain NaN"),
                    },
                    2 => SensorMetric::AirTemp {
                        value: NotNan::new(s_row.try_get::<f64, _>("metric_value")?)
                            .expect("database should not contain NaN"),
                    },
                    3 => SensorMetric::Humidity {
                        value: Percentage(s_row.try_get::<f64, _>("metric_value")? as u8),
                    },
                    4 => SensorMetric::Rainfall {
                        value: NotNan::new(s_row.try_get::<f64, _>("metric_value")?)
                            .expect("database should not contain NaN"),
                    },
                    other => return Err(Self::Error::InvalidMetricType(other)),
                },
            });
        }

        let provisioned_at = r.try_get::<i64, _>("provisioned_at")?;
        let provisioned_at = jiff::Timestamp::from_second(provisioned_at)
            .map_err(|_| Self::Error::InvalidTimestamp(provisioned_at))?;

        let state = match r.try_get::<i32, _>("state")? {
            0 => DeviceState::Active,
            1 => DeviceState::Suspended,
            other => return Err(Self::Error::InvalidState(other)),
        };

        let kind = match r.try_get::<i32, _>("kind")? {
            0 => DeviceKind::Sensor,
            other => return Err(Self::Error::InvalidDeviceKind(other)),
        };

        let manufacturer = r
            .try_get::<Option<String>, _>("manufacturer")?
            .map(|s| s.into_boxed_str());

        Ok(Some(Device {
            id,
            kind,
            state,
            location: H3Cell(r.try_get::<i64, _>("location")? as u64),
            manufacturer,
            provisioned_at,
            sensors: sensors.into_boxed_slice(),
        }))
    }

    async fn update(&self, id: DeviceId, new: Device) -> Result<(), Self::Error> {
        let old = self.get(id).await?.ok_or(Self::Error::NotFound)?;
        let new = Device { id: old.id, ..new };

        self.register(new).await
    }

    async fn suspend(&self, id: DeviceId) -> Result<(), Self::Error> {
        let device = self.get(id).await?.ok_or(Self::Error::NotFound)?;

        let new = Device {
            state: DeviceState::Suspended,
            ..device
        };

        self.register(new).await
    }

    async fn batch_register(&self, devices: Vec<Device>) -> Result<(), Self::Error> {
        let mut tx = self.pool.begin().await?;

        for device in devices {
            sqlx::query(
                r#"
            INSERT OR REPLACE INTO devices (id, kind, state, location, manufacturer, provisioned_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            )
            .bind(device.id.0.to_string())
            .bind(device.kind as i32)
            .bind(device.state as i32)
            .bind(device.location.0 as i64)
            .bind(device.manufacturer)
            .bind(device.provisioned_at.as_second())
            .execute(&mut *tx)
            .await?;

            self.add_sensors(device.id, device.sensors.into_iter())
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn count(&self, filter: Option<DeviceFilter>) -> Result<usize, Self::Error> {
        let mut query_builder = QueryBuilder::new("SELECT COUNT(*) FROM devices ");

        if let Some(filter) = filter {
            query_builder = filter_devices(query_builder, filter);
        }

        let query = query_builder.build();
        let count: i64 = query.fetch_one(&self.pool).await?.try_get(0)?;

        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<DeviceFilter, DeviceSortBy>,
    ) -> Result<Vec<Device>, Self::Error> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, kind, state, location, manufacturer, provisioned_at, sensor_count FROM devices ",
        );

        query_builder = filter_devices(query_builder, options.filter);

        query_builder.push(match options.sort_by {
            DeviceSortBy::State => " ORDER BY state",
            DeviceSortBy::Manufacturer => " ORDER BY manufacturer",
            DeviceSortBy::ProvisionAt => " ORDER BY provisioned_at ",
            DeviceSortBy::SensorCount => " ORDER BY sensor_count",
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
                // Note: Efficient cursor pagination usually requires modifying
                // the WHERE clause. For simple usage, we apply LIMIT here.
                query_builder.push(" LIMIT ").push_bind(limit as i64);
            }
        }

        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        if rows.is_empty() {
            return Ok(vec![]);
        }

        let mut devices = Vec::with_capacity(rows.len());
        let mut device_ids = Vec::with_capacity(rows.len());

        for row in rows {
            let id_str: String = row.try_get("id")?;
            device_ids.push(id_str.clone());

            devices.push(map_row_to_device(row)?);
        }

        let mut sensor_query = QueryBuilder::new(
            "SELECT id, kind, metric_type, metric_value, device_id FROM sensors WHERE device_id IN (",
        );
        let mut separated = sensor_query.separated(", ");
        for id in &device_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let sensor_rows = sensor_query.build().fetch_all(&self.pool).await?;

        for s_row in sensor_rows {
            let d_id: String = s_row.try_get("device_id")?;
            let sensor = map_row_to_sensor(s_row)?;

            if let Some(device) = devices.iter_mut().find(|d| d.id.0.to_string() == d_id) {
                let mut v = device.sensors.clone().into_vec();
                v.push(sensor);
                device.sensors = v.into_boxed_slice();
            }
        }

        Ok(devices)
    }
}

fn map_row_to_device(r: SqliteRow) -> Result<Device, SqliteDeviceError> {
    let id_str: String = r.try_get("id")?;
    let ulid = Ulid::from_str(&id_str).map_err(|_| SqliteDeviceError::InvalidUlid(id_str))?;

    let provisioned_at: i64 = r.try_get("provisioned_at")?;

    Ok(Device {
        id: DeviceId(ulid),
        kind: match r.try_get::<i32, _>("kind")? {
            0 => DeviceKind::Sensor,
            other => return Err(SqliteDeviceError::InvalidDeviceKind(other)),
        },
        state: match r.try_get::<i32, _>("state")? {
            0 => DeviceState::Active,
            1 => DeviceState::Suspended,
            other => return Err(SqliteDeviceError::InvalidState(other)),
        },
        location: H3Cell(r.try_get::<i64, _>("location")? as u64),
        manufacturer: r
            .try_get::<Option<String>, _>("manufacturer")?
            .map(|s| s.into_boxed_str()),
        provisioned_at: jiff::Timestamp::from_second(provisioned_at).unwrap(),
        sensors: vec![].into_boxed_slice(),
    })
}

fn map_row_to_sensor(row: SqliteRow) -> Result<Sensor, SqliteDeviceError> {
    let id_str: String = row.try_get("id")?;
    let ulid = Ulid::from_str(&id_str).map_err(|_| SqliteDeviceError::InvalidUlid(id_str))?;

    let kind_int: i32 = row.try_get("kind")?;
    let metric_type: i32 = row.try_get("metric_type")?;
    let metric_value: f64 = row.try_get("metric_value")?;

    let kind = match kind_int {
        0 => SensorKind::SoilMoisture,
        1 => SensorKind::SoilTemp,
        2 => SensorKind::AirTemp,
        3 => SensorKind::Humidity,
        4 => SensorKind::Rainfall,
        other => return Err(SqliteDeviceError::InvalidSensorKind(other)),
    };

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
        other => return Err(SqliteDeviceError::InvalidMetricType(other)),
    };

    Ok(Sensor {
        id: SensorId(ulid),
        kind,
        metric,
    })
}

fn filter_devices<'a>(
    mut query_builder: QueryBuilder<'a, Sqlite>,
    filter: DeviceFilter,
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

    if let Some(states) = filter.states
        && !states.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("state IN (");
        let mut separated = query_builder.separated(", ");
        for state in states {
            let val = match state {
                DeviceState::Active => 0,
                DeviceState::Suspended => 1,
            };
            separated.push_bind(val);
        }
        separated.push_unseparated(")");
    }

    if let Some(kinds) = filter.kinds
        && !kinds.is_empty()
    {
        prefix(&mut query_builder);
        query_builder.push("kind IN (");
        let mut separated = query_builder.separated(", ");
        for kind in kinds {
            let val = match kind {
                DeviceKind::Sensor => 0,
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

    if let Some(after) = filter.provisioned_after {
        prefix(&mut query_builder);
        query_builder
            .push("provisioned_at >= ")
            .push_bind(after.as_second());
    }

    if let Some(before) = filter.provisioned_before {
        prefix(&mut query_builder);
        query_builder
            .push("provisioned_at <= ")
            .push_bind(before.as_second());
    }

    if let Some(range) = filter.sensor_count {
        prefix(&mut query_builder);

        query_builder
            .push(" sensor_count BETWEEN ")
            .push_bind(*range.start() as i64)
            .push(" AND ")
            .push_bind(*range.end() as i64);
    }

    if let Some(pattern) = filter.manufacturer_pattern {
        prefix(&mut query_builder);
        query_builder
            .push("manufacturer LIKE ")
            .push_bind(format!("%{}%", pattern));
    }

    query_builder
}

fn disect_metric(metric: SensorMetric) -> (i32, f64) {
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
    use ersha_core::Percentage;
    use ordered_float::NotNan;
    use ulid::Ulid;

    use crate::registry::DeviceRegistry;
    use crate::registry::filter::{
        DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{
        Device, DeviceId, DeviceKind, DeviceState, H3Cell, Sensor, SensorId, SensorKind,
        SensorMetric,
    };

    use super::SqliteDeviceRegistry;

    fn mock_device(id: Ulid) -> Device {
        Device {
            id: DeviceId(id),
            kind: DeviceKind::Sensor,
            state: DeviceState::Active,
            location: H3Cell(0x8a2a1072b59ffff),
            manufacturer: Some("TestCorp".to_string().into_boxed_str()),
            provisioned_at: jiff::Timestamp::now(),
            sensors: vec![Sensor {
                id: SensorId(Ulid::new()),
                kind: SensorKind::AirTemp,
                metric: SensorMetric::AirTemp {
                    value: NotNan::new(22.5).unwrap(),
                },
            }]
            .into_boxed_slice(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get_device() {
        let registry = SqliteDeviceRegistry::new_in_memory().await.unwrap();
        let id = Ulid::new();
        let device = mock_device(id);

        registry
            .register(device.clone())
            .await
            .expect("Should register");

        let fetched = registry
            .get(DeviceId(id))
            .await
            .expect("Should fetch")
            .expect("Device should exist");

        assert_eq!(fetched.id, device.id);
        assert_eq!(fetched.manufacturer, device.manufacturer);
        assert_eq!(fetched.sensors.len(), 1);
        assert!(
            matches!(fetched.sensors[0].metric, SensorMetric::AirTemp { value } if value == 22.5)
        );
    }

    #[tokio::test]
    async fn test_filter_by_manufacturer() {
        let registry = SqliteDeviceRegistry::new_in_memory().await.unwrap();

        let mut d1 = mock_device(Ulid::new());
        d1.manufacturer = Some("Apple".to_string().into_boxed_str());

        let mut d2 = mock_device(Ulid::new());
        d2.manufacturer = Some("Banana".to_string().into_boxed_str());

        registry.register(d1).await.unwrap();
        registry.register(d2).await.unwrap();

        let filter = DeviceFilter {
            manufacturer_pattern: Some("App".to_string()),
            ..Default::default()
        };

        let options = QueryOptions {
            filter,
            sort_by: DeviceSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manufacturer.as_deref(), Some("Apple"));
    }

    #[tokio::test]
    async fn test_suspend_device() {
        let registry = SqliteDeviceRegistry::new_in_memory().await.unwrap();

        let id = Ulid::new();
        let device = mock_device(id);

        registry.register(device).await.unwrap();
        registry.suspend(DeviceId(id)).await.unwrap();

        let fetched = registry.get(DeviceId(id)).await.unwrap().unwrap();
        assert_eq!(fetched.state, DeviceState::Suspended);
    }

    #[tokio::test]
    async fn test_add_sensor_individually() {
        let registry = SqliteDeviceRegistry::new_in_memory().await.unwrap();

        let d_id = Ulid::new();
        let mut device = mock_device(d_id);
        device.sensors = vec![].into_boxed_slice(); // Start with none

        registry.register(device).await.unwrap();

        let new_sensor = Sensor {
            id: SensorId(Ulid::new()),
            kind: SensorKind::Humidity,
            metric: SensorMetric::Humidity {
                value: Percentage(45),
            },
        };

        registry
            .add_sensor(DeviceId(d_id), new_sensor)
            .await
            .unwrap();

        let fetched = registry.get(DeviceId(d_id)).await.unwrap().unwrap();
        assert_eq!(fetched.sensors.len(), 1);
        assert!(matches!(fetched.sensors[0].kind, SensorKind::Humidity));
    }
}
