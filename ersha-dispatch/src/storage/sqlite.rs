use async_trait::async_trait;
use sqlx::{Error as SqlxError, Row, SqlitePool};
use std::fmt;
use std::path::Path;
use std::time::Duration;

use crate::storage::migrations::Migrator;
use crate::storage::{CleanupStats, Storage, StorageStats};
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

#[derive(Clone)]
pub struct SqliteStorage {
    pool: SqlitePool,
}

#[derive(Debug)]
pub enum SqliteStorageError {
    ConnectionFailed(String),
    SchemaCreationFailed(String),
    SerializationFailed(String),
    DeserializationFailed(String),
    QueryFailed(String),
    TransactionFailed(String),
    UpdateFailed(String),
    RowProcessingFailed(String),
    TimeConversionFailed(String),
    PoolError(String),
}

impl std::error::Error for SqliteStorageError {}

impl fmt::Display for SqliteStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqliteStorageError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            SqliteStorageError::SchemaCreationFailed(msg) => {
                write!(f, "Schema creation failed: {}", msg)
            }
            SqliteStorageError::SerializationFailed(msg) => {
                write!(f, "Serialization failed: {}", msg)
            }
            SqliteStorageError::DeserializationFailed(msg) => {
                write!(f, "Deserialization failed: {}", msg)
            }
            SqliteStorageError::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            SqliteStorageError::TransactionFailed(msg) => write!(f, "Transaction failed: {}", msg),
            SqliteStorageError::UpdateFailed(msg) => write!(f, "Update failed: {}", msg),
            SqliteStorageError::RowProcessingFailed(msg) => {
                write!(f, "Row processing failed: {}", msg)
            }
            SqliteStorageError::TimeConversionFailed(msg) => {
                write!(f, "Time conversion failed: {}", msg)
            }
            SqliteStorageError::PoolError(msg) => write!(f, "Pool error: {}", msg),
        }
    }
}

impl From<SqlxError> for SqliteStorageError {
    fn from(err: SqlxError) -> Self {
        SqliteStorageError::QueryFailed(err.to_string())
    }
}

impl From<serde_json::Error> for SqliteStorageError {
    fn from(err: serde_json::Error) -> Self {
        SqliteStorageError::SerializationFailed(err.to_string())
    }
}

impl SqliteStorage {
    /// create new SQLite storage with automatic migrations
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, SqliteStorageError> {
        let database_url = format!("sqlite:{}", path.as_ref().display());
        let pool = SqlitePool::connect(&database_url).await.map_err(|e| {
            SqliteStorageError::ConnectionFailed(format!("Failed to connect to SQLite DB: {}", e))
        })?;

        // enable WAL for better concurrency
        sqlx::query("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .execute(&pool)
            .await?;

        // run migrations first
        Self::run_migrations(&pool).await?;

        // ensure all indexes exist
        Self::ensure_indexes(&pool).await?;

        Ok(Self { pool })
    }

    /// run database migrations
    async fn run_migrations(pool: &SqlitePool) -> Result<(), SqliteStorageError> {
        Migrator::run_migrations(pool).await.map_err(|e| {
            SqliteStorageError::SchemaCreationFailed(format!("Migration failed: {}", e))
        })
    }

    /// ensure all necessary indexes exist
    async fn ensure_indexes(pool: &SqlitePool) -> Result<(), SqliteStorageError> {
        sqlx::query(
            r#"
            -- Ensure all indexes exist (idempotent)
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_state ON sensor_readings(state);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_state ON device_statuses(state);
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_uploaded_at ON sensor_readings(uploaded_at);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_uploaded_at ON device_statuses(uploaded_at);
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_created_at ON sensor_readings(created_at);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_created_at ON device_statuses(created_at);
            "#,
        )
            .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_version(&self) -> Result<i64, SqliteStorageError> {
        Migrator::get_version(&self.pool)
            .await
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Failed to get version: {}", e)))
    }

    pub async fn check_schema(&self) -> Result<bool, SqliteStorageError> {
        let version = self.get_version().await?;
        Ok(version >= 1) // version 1 is our current schema
    }

    fn serialize_reading(reading: &SensorReading) -> Result<String, serde_json::Error> {
        serde_json::to_string(reading)
    }

    fn deserialize_reading(json: &str) -> Result<SensorReading, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn serialize_status(status: &DeviceStatus) -> Result<String, serde_json::Error> {
        serde_json::to_string(status)
    }

    fn deserialize_status(json: &str) -> Result<DeviceStatus, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    type Error = SqliteStorageError;

    async fn store_sensor_reading(&self, reading: SensorReading) -> Result<(), Self::Error> {
        let json = Self::serialize_reading(&reading)?;
        let id_str = reading.id.0.to_string();

        sqlx::query(
            "INSERT INTO sensor_readings (id, reading_json, state) VALUES (?, ?, 'pending')",
        )
        .bind(&id_str)
        .bind(&json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn store_device_status(&self, status: DeviceStatus) -> Result<(), Self::Error> {
        let json = Self::serialize_status(&status)?;
        let id_str = status.id.0.to_string();

        sqlx::query(
            "INSERT INTO device_statuses (id, status_json, state) VALUES (?, ?, 'pending')",
        )
        .bind(&id_str)
        .bind(&json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn store_sensor_readings_batch(
        &self,
        readings: Vec<SensorReading>,
    ) -> Result<(), Self::Error> {
        if readings.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
        })?;

        for reading in readings {
            let json = Self::serialize_reading(&reading)?;
            let id_str = reading.id.0.to_string();

            sqlx::query(
                "INSERT INTO sensor_readings (id, reading_json, state) VALUES (?, ?, 'pending')",
            )
            .bind(&id_str)
            .bind(&json)
            .execute(&mut *tx)
            .await
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Insert failed: {}", e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn store_device_statuses_batch(
        &self,
        statuses: Vec<DeviceStatus>,
    ) -> Result<(), Self::Error> {
        if statuses.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
        })?;

        for status in statuses {
            let json = Self::serialize_status(&status)?;
            let id_str = status.id.0.to_string();

            sqlx::query(
                "INSERT INTO device_statuses (id, status_json, state) VALUES (?, ?, 'pending')",
            )
            .bind(&id_str)
            .bind(&json)
            .execute(&mut *tx)
            .await
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Insert failed: {}", e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error> {
        let rows = sqlx::query("SELECT reading_json FROM sensor_readings WHERE state = 'pending'")
            .fetch_all(&self.pool)
            .await?;

        let mut readings = Vec::new();
        for row in rows {
            let json: String = row.try_get("reading_json").map_err(|e| {
                SqliteStorageError::RowProcessingFailed(format!("Failed to get column: {}", e))
            })?;
            let reading = Self::deserialize_reading(&json)?;
            readings.push(reading);
        }

        Ok(readings)
    }

    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error> {
        let rows = sqlx::query("SELECT status_json FROM device_statuses WHERE state = 'pending'")
            .fetch_all(&self.pool)
            .await?;

        let mut statuses = Vec::new();
        for row in rows {
            let json: String = row.try_get("status_json").map_err(|e| {
                SqliteStorageError::RowProcessingFailed(format!("Failed to get column: {}", e))
            })?;
            let status = Self::deserialize_status(&json)?;
            statuses.push(status);
        }

        Ok(statuses)
    }

    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
        })?;

        for id in ids {
            let id_str = id.0.to_string();

            sqlx::query(
                "UPDATE sensor_readings SET state = 'uploaded', uploaded_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
                .bind(&id_str)
                .execute(&mut *tx)
                .await
                .map_err(|e| SqliteStorageError::UpdateFailed(format!("Update failed for {}: {}", id_str, e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn mark_device_statuses_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
        })?;

        for id in ids {
            let id_str = id.0.to_string();

            sqlx::query(
                "UPDATE device_statuses SET state = 'uploaded', uploaded_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
                .bind(&id_str)
                .execute(&mut *tx)
                .await
                .map_err(|e| SqliteStorageError::UpdateFailed(format!("Update failed for {}: {}", id_str, e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn get_stats(&self) -> Result<StorageStats, Self::Error> {
        let sensor_stats: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT 
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN state = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN state = 'uploaded' THEN 1 ELSE 0 END), 0) as uploaded
             FROM sensor_readings
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let device_stats: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT 
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN state = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN state = 'uploaded' THEN 1 ELSE 0 END), 0) as uploaded
             FROM device_statuses
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(StorageStats {
            sensor_readings_total: sensor_stats.0 as usize,
            sensor_readings_pending: sensor_stats.1 as usize,
            sensor_readings_uploaded: sensor_stats.2 as usize,
            device_statuses_total: device_stats.0 as usize,
            device_statuses_pending: device_stats.1 as usize,
            device_statuses_uploaded: device_stats.2 as usize,
        })
    }

    async fn cleanup_uploaded(&self, older_than: Duration) -> Result<CleanupStats, Self::Error> {
        if older_than == Duration::ZERO {
            let mut tx = self.pool.begin().await.map_err(|e| {
                SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
            })?;

            let sensor_deleted =
                sqlx::query("DELETE FROM sensor_readings WHERE state = 'uploaded'")
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();

            let device_deleted =
                sqlx::query("DELETE FROM device_statuses WHERE state = 'uploaded'")
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();

            tx.commit().await.map_err(|e| {
                SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e))
            })?;

            return Ok(CleanupStats {
                sensor_readings_deleted: sensor_deleted as usize,
                device_statuses_deleted: device_deleted as usize,
            });
        }

        let cutoff_days = older_than.as_secs_f64() / 86400.0;

        let mut tx = self.pool.begin().await.map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Failed to begin transaction: {}", e))
        })?;

        let sensor_deleted = sqlx::query(
            "DELETE FROM sensor_readings WHERE state = 'uploaded' AND uploaded_at IS NOT NULL AND julianday('now') - julianday(uploaded_at) >= ?",
        )
            .bind(cutoff_days)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        let device_deleted = sqlx::query(
            "DELETE FROM device_statuses WHERE state = 'uploaded' AND uploaded_at IS NOT NULL AND julianday('now') - julianday(uploaded_at) >= ?",
        )
            .bind(cutoff_days)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        tx.commit()
            .await
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(CleanupStats {
            sensor_readings_deleted: sensor_deleted as usize,
            device_statuses_deleted: device_deleted as usize,
        })
    }
}
