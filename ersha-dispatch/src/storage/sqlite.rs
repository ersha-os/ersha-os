use async_trait::async_trait;
use rusqlite::{params, Connection, Error as RusqliteError, Row};
use serde_json::Error as SerdeJsonError;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::storage::migrations::Migrator;
use crate::storage::{CleanupStats, Storage, StorageStats};
use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};

#[derive(Clone)]
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
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
        }
    }
}

impl From<RusqliteError> for SqliteStorageError {
    fn from(err: RusqliteError) -> Self {
        SqliteStorageError::QueryFailed(err.to_string())
    }
}

impl From<SerdeJsonError> for SqliteStorageError {
    fn from(err: SerdeJsonError) -> Self {
        SqliteStorageError::SerializationFailed(err.to_string())
    }
}

impl SqliteStorage {
    /// Create new SQLite storage with automatic migrations
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, SqliteStorageError> {
        let conn = Connection::open(path).map_err(|e| {
            SqliteStorageError::ConnectionFailed(format!("Failed to open SQLite DB: {}", e))
        })?;

        // Enable WAL for better concurrency
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;

        // Run migrations first
        Self::run_migrations(&conn)?;

        // Ensure all indexes exist
        Self::ensure_indexes(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run database migrations
    fn run_migrations(conn: &Connection) -> Result<(), SqliteStorageError> {
        Migrator::run_migrations(conn).map_err(|e| {
            SqliteStorageError::SchemaCreationFailed(format!("Migration failed: {}", e))
        })
    }

    /// Ensure all necessary indexes exist
    fn ensure_indexes(conn: &Connection) -> Result<(), SqliteStorageError> {
        conn.execute_batch(
            r#"
            -- Ensure all indexes exist (idempotent)
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_state ON sensor_readings(state);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_state ON device_statuses(state);
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_uploaded_at ON sensor_readings(uploaded_at);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_uploaded_at ON device_statuses(uploaded_at);
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_created_at ON sensor_readings(created_at);
            CREATE INDEX IF NOT EXISTS idx_device_statuses_created_at ON device_statuses(created_at);
            "#,
        )?;
        Ok(())
    }

    pub async fn get_version(&self) -> Result<i64, SqliteStorageError> {
        let conn = self.conn.lock().await;
        Migrator::get_version(&conn)
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Failed to get version: {}", e)))
    }

    pub async fn check_schema(&self) -> Result<bool, SqliteStorageError> {
        let version = self.get_version().await?;
        Ok(version >= 1) // version 1 is our current schema
    }

    fn serialize_reading(reading: &SensorReading) -> Result<String, SerdeJsonError> {
        serde_json::to_string(reading)
    }

    fn deserialize_reading(json: &str) -> Result<SensorReading, SerdeJsonError> {
        serde_json::from_str(json)
    }

    fn serialize_status(status: &DeviceStatus) -> Result<String, SerdeJsonError> {
        serde_json::to_string(status)
    }

    fn deserialize_status(json: &str) -> Result<DeviceStatus, SerdeJsonError> {
        serde_json::from_str(json)
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    type Error = SqliteStorageError;

    async fn store_sensor_reading(&self, reading: SensorReading) -> Result<(), Self::Error> {
        let json = Self::serialize_reading(&reading)?;
        let id_str = reading.id.0.to_string();

        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO sensor_readings (id, reading_json, state) VALUES (?, ?, 'pending')",
            params![id_str, json],
        )?;

        Ok(())
    }

    async fn store_device_status(&self, status: DeviceStatus) -> Result<(), Self::Error> {
        let json = Self::serialize_status(&status)?;
        let id_str = status.id.0.to_string();

        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO device_statuses (id, status_json, state) VALUES (?, ?, 'pending')",
            params![id_str, json],
        )?;

        Ok(())
    }

    async fn store_sensor_readings_batch(
        &self,
        readings: Vec<SensorReading>,
    ) -> Result<(), Self::Error> {
        if readings.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().await;
        let tx = conn.unchecked_transaction().map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
        })?;

        for reading in readings {
            let json = Self::serialize_reading(&reading)?;
            let id_str = reading.id.0.to_string();

            tx.execute(
                "INSERT INTO sensor_readings (id, reading_json, state) VALUES (?, ?, 'pending')",
                params![id_str, json],
            )
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Insert failed: {}", e)))?;
        }

        tx.commit()
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

        let conn = self.conn.lock().await;
        let tx = conn.unchecked_transaction().map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
        })?;

        for status in statuses {
            let json = Self::serialize_status(&status)?;
            let id_str = status.id.0.to_string();

            tx.execute(
                "INSERT INTO device_statuses (id, status_json, state) VALUES (?, ?, 'pending')",
                params![id_str, json],
            )
            .map_err(|e| SqliteStorageError::QueryFailed(format!("Insert failed: {}", e)))?;
        }

        tx.commit()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error> {
        let conn = self.conn.lock().await;

        let mut stmt =
            conn.prepare("SELECT reading_json FROM sensor_readings WHERE state = 'pending'")?;

        let rows = stmt.query_map([], |row: &Row| {
            let json: String = row.get(0)?;
            Ok(json)
        })?;

        let mut readings = Vec::new();
        for row_result in rows {
            let json = row_result.map_err(|e| {
                SqliteStorageError::RowProcessingFailed(format!("Row error: {}", e))
            })?;
            let reading = Self::deserialize_reading(&json).map_err(|e| {
                SqliteStorageError::DeserializationFailed(format!("Deserialization error: {}", e))
            })?;
            readings.push(reading);
        }

        Ok(readings)
    }

    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error> {
        let conn = self.conn.lock().await;

        let mut stmt =
            conn.prepare("SELECT status_json FROM device_statuses WHERE state = 'pending'")?;

        let rows = stmt.query_map([], |row: &Row| {
            let json: String = row.get(0)?;
            Ok(json)
        })?;

        let mut statuses = Vec::new();
        for row_result in rows {
            let json = row_result.map_err(|e| {
                SqliteStorageError::RowProcessingFailed(format!("Row error: {}", e))
            })?;
            let status = Self::deserialize_status(&json).map_err(|e| {
                SqliteStorageError::DeserializationFailed(format!("Deserialization error: {}", e))
            })?;
            statuses.push(status);
        }

        Ok(statuses)
    }

    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error> {
        if ids.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().await;
        let tx = conn.unchecked_transaction().map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
        })?;

        for id in ids {
            let id_str = id.0.to_string();
            tx.execute(
                "UPDATE sensor_readings SET state = 'uploaded', uploaded_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![id_str],
            )
            .map_err(|e| SqliteStorageError::UpdateFailed(format!("Update failed for {}: {}", id_str, e)))?;
        }

        tx.commit()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn mark_device_statuses_uploaded(&self, ids: &[StatusId]) -> Result<(), Self::Error> {
        if ids.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().await;
        let tx = conn.unchecked_transaction().map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
        })?;

        for id in ids {
            let id_str = id.0.to_string();
            tx.execute(
                "UPDATE device_statuses SET state = 'uploaded', uploaded_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![id_str],
            )
            .map_err(|e| SqliteStorageError::UpdateFailed(format!("Update failed for {}: {}", id_str, e)))?;
        }

        tx.commit()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    async fn get_stats(&self) -> Result<StorageStats, Self::Error> {
        let conn = self.conn.lock().await;

        let sensor_stats = conn.query_row(
            "SELECT 
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN state = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN state = 'uploaded' THEN 1 ELSE 0 END), 0) as uploaded
             FROM sensor_readings",
            [],
            |row| {
                let total: i64 = row.get(0)?;
                let pending: i64 = row.get(1)?;
                let uploaded: i64 = row.get(2)?;
                Ok((total as usize, pending as usize, uploaded as usize))
            },
        )?;

        let device_stats = conn.query_row(
            "SELECT 
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN state = 'pending' THEN 1 ELSE 0 END), 0) as pending,
                COALESCE(SUM(CASE WHEN state = 'uploaded' THEN 1 ELSE 0 END), 0) as uploaded
             FROM device_statuses",
            [],
            |row| {
                let total: i64 = row.get(0)?;
                let pending: i64 = row.get(1)?;
                let uploaded: i64 = row.get(2)?;
                Ok((total as usize, pending as usize, uploaded as usize))
            },
        )?;

        Ok(StorageStats {
            sensor_readings_total: sensor_stats.0,
            sensor_readings_pending: sensor_stats.1,
            sensor_readings_uploaded: sensor_stats.2,
            device_statuses_total: device_stats.0,
            device_statuses_pending: device_stats.1,
            device_statuses_uploaded: device_stats.2,
        })
    }

    async fn cleanup_uploaded(&self, older_than: Duration) -> Result<CleanupStats, Self::Error> {
        let conn = self.conn.lock().await;

        if older_than == Duration::ZERO {
            let tx = conn.unchecked_transaction().map_err(|e| {
                SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
            })?;

            let sensor_deleted = tx
                .execute("DELETE FROM sensor_readings WHERE state = 'uploaded'", [])
                .map_err(|e| SqliteStorageError::QueryFailed(format!("Delete failed: {}", e)))?;

            let device_deleted = tx
                .execute("DELETE FROM device_statuses WHERE state = 'uploaded'", [])
                .map_err(|e| SqliteStorageError::QueryFailed(format!("Delete failed: {}", e)))?;

            tx.commit().map_err(|e| {
                SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e))
            })?;

            return Ok(CleanupStats {
                sensor_readings_deleted: sensor_deleted,
                device_statuses_deleted: device_deleted,
            });
        }

        let cutoff_days = older_than.as_secs_f64() / 86400.0;

        let tx = conn.unchecked_transaction().map_err(|e| {
            SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e))
        })?;

        let sensor_deleted = tx.execute(
            "DELETE FROM sensor_readings WHERE state = 'uploaded' AND uploaded_at IS NOT NULL AND julianday('now') - julianday(uploaded_at) >= ?",
            params![cutoff_days],
        )
        .map_err(|e| SqliteStorageError::QueryFailed(format!("Delete failed: {}", e)))?;

        let device_deleted = tx.execute(
            "DELETE FROM device_statuses WHERE state = 'uploaded' AND uploaded_at IS NOT NULL AND julianday('now') - julianday(uploaded_at) >= ?",
            params![cutoff_days],
        )
        .map_err(|e| SqliteStorageError::QueryFailed(format!("Delete failed: {}", e)))?;

        tx.commit()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;

        Ok(CleanupStats {
            sensor_readings_deleted: sensor_deleted,
            device_statuses_deleted: device_deleted,
        })
    }
}
