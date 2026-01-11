use std::str::FromStr;

use ersha_core::{
    Device, DeviceId, DeviceKind, DeviceState, H3Cell, Percentage, Sensor, SensorId, SensorKind,
    SensorMetric,
};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, sqlite::SqliteRow};
use ulid::Ulid;

use crate::registry::{
    DeviceRegistry,
    filter::{DeviceFilter, DeviceSortBy, Pagination, QueryOptions, SortOrder},
};

#[derive(Debug)]
pub enum SqliteDeviceError {
    Sqlx(sqlx::Error),
    InvalidUlid(String),
    InvalidTimestamp(i64),
    InvalidState(i32),
    InvalidDeviceKind(i32),
    InvalidMetricType(i32),
    InvalidSensorKind(i32),
    NotFound,
}

impl From<sqlx::Error> for SqliteDeviceError {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}

pub struct SqliteDeviceRegistry {
    pub pool: SqlitePool,
}

impl SqliteDeviceRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl DeviceRegistry for SqliteDeviceRegistry {
    type Error = SqliteDeviceError;

    async fn register(&mut self, device: Device) -> Result<(), Self::Error> {
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

    async fn add_sensor(&mut self, id: DeviceId, sensor: Sensor) -> Result<(), Self::Error> {
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
        &mut self,
        id: DeviceId,
        sensors: impl Iterator<Item = Sensor>,
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
                        value: s_row.try_get::<f64, _>("metric_value")?,
                    },
                    2 => SensorMetric::AirTemp {
                        value: s_row.try_get::<f64, _>("metric_value")?,
                    },
                    3 => SensorMetric::Humidity {
                        value: Percentage(s_row.try_get::<f64, _>("metric_value")? as u8),
                    },
                    4 => SensorMetric::Rainfall {
                        value: s_row.try_get::<f64, _>("metric_value")?,
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

    async fn update(&mut self, id: DeviceId, new: Device) -> Result<(), Self::Error> {
        let old = self.get(id).await?.ok_or(Self::Error::NotFound)?;
        let new = Device { id: old.id, ..new };

        self.register(new).await
    }

    async fn suspend(&mut self, id: DeviceId) -> Result<(), Self::Error> {
        let device = self.get(id).await?.ok_or(Self::Error::NotFound)?;

        let new = Device {
            state: DeviceState::Suspended,
            ..device
        };

        self.register(new).await
    }

    async fn batch_register(&mut self, devices: Vec<Device>) -> Result<(), Self::Error> {
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
            value: metric_value,
        },
        2 => SensorMetric::AirTemp {
            value: metric_value,
        },
        3 => SensorMetric::Humidity {
            value: Percentage(metric_value as u8),
        },
        4 => SensorMetric::Rainfall {
            value: metric_value,
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
        SensorMetric::SoilTemp { value } => (1, value),
        SensorMetric::AirTemp { value } => (2, value),
        SensorMetric::Humidity { value } => (3, value.0 as f64),
        SensorMetric::Rainfall { value } => (4, value),
    }
}
