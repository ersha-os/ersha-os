use async_trait::async_trait;
use rusqlite::{params, Connection, Error as RusqliteError, Row};
use serde_json::Error as SerdeJsonError;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use ersha_core::{DeviceStatus, ReadingId, SensorReading, StatusId};
use crate::storage::Storage;

/// SQLite-backed storage implementation.
/// Stores events as JSON blobs in two tables.
#[derive(Clone)]
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

/// Error type for SqliteStorage
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
}

impl std::error::Error for SqliteStorageError {}

impl fmt::Display for SqliteStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqliteStorageError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            SqliteStorageError::SchemaCreationFailed(msg) => write!(f, "Schema creation failed: {}", msg),
            SqliteStorageError::SerializationFailed(msg) => write!(f, "Serialization failed: {}", msg),
            SqliteStorageError::DeserializationFailed(msg) => write!(f, "Deserialization failed: {}", msg),
            SqliteStorageError::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            SqliteStorageError::TransactionFailed(msg) => write!(f, "Transaction failed: {}", msg),
            SqliteStorageError::UpdateFailed(msg) => write!(f, "Update failed: {}", msg),
            SqliteStorageError::RowProcessingFailed(msg) => write!(f, "Row processing failed: {}", msg),
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
    /// opens or creates a SQLite database at the given path.
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, SqliteStorageError> {
        let conn = Connection::open(path)
            .map_err(|e| SqliteStorageError::ConnectionFailed(format!("Failed to open SQLite DB: {}", e)))?;
        
        // initialize database schema
        Self::init_schema(&conn)?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    fn init_schema(conn: &Connection) -> Result<(), SqliteStorageError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id TEXT PRIMARY KEY,
                reading_json TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE TABLE IF NOT EXISTS device_statuses (
                id TEXT PRIMARY KEY,
                status_json TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_state 
            ON sensor_readings(state);
            
            CREATE INDEX IF NOT EXISTS idx_device_statuses_state 
            ON device_statuses(state);
            "#,
        )
        .map_err(|e| SqliteStorageError::SchemaCreationFailed(format!("Failed to create tables: {}", e)))?;
        
        Ok(())
    }
    
    /// serialize SensorReading to JSON string
    fn serialize_reading(reading: &SensorReading) -> Result<String, SerdeJsonError> {
        serde_json::to_string(reading)
    }
    
    /// deserialize JSON string to SensorReading
    fn deserialize_reading(json: &str) -> Result<SensorReading, SerdeJsonError> {
        serde_json::from_str(json)
    }
    
    /// serialize DeviceStatus to JSON string
    fn serialize_status(status: &DeviceStatus) -> Result<String, SerdeJsonError> {
        serde_json::to_string(status)
    }
    
    /// deserialize JSON string to DeviceStatus
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
    
    async fn fetch_pending_sensor_readings(&self) -> Result<Vec<SensorReading>, Self::Error> {
        let conn = self.conn.lock().await;
        
        let mut stmt = conn
            .prepare("SELECT reading_json FROM sensor_readings WHERE state = 'pending'")?;
        
        let rows = stmt
            .query_map([], |row: &Row| {
                let json: String = row.get(0)?;
                Ok(json)
            })?;
        
        let mut readings = Vec::new();
        for row_result in rows {
            let json = row_result
                .map_err(|e| SqliteStorageError::RowProcessingFailed(format!("Row error: {}", e)))?;
            let reading = Self::deserialize_reading(&json)
                .map_err(|e| SqliteStorageError::DeserializationFailed(format!("Deserialization error: {}", e)))?;
            readings.push(reading);
        }
        
        Ok(readings)
    }
    
    async fn fetch_pending_device_statuses(&self) -> Result<Vec<DeviceStatus>, Self::Error> {
        let conn = self.conn.lock().await;
        
        let mut stmt = conn
            .prepare("SELECT status_json FROM device_statuses WHERE state = 'pending'")?;
        
        let rows = stmt
            .query_map([], |row: &Row| {
                let json: String = row.get(0)?;
                Ok(json)
            })?;
        
        let mut statuses = Vec::new();
        for row_result in rows {
            let json = row_result
                .map_err(|e| SqliteStorageError::RowProcessingFailed(format!("Row error: {}", e)))?;
            let status = Self::deserialize_status(&json)
                .map_err(|e| SqliteStorageError::DeserializationFailed(format!("Deserialization error: {}", e)))?;
            statuses.push(status);
        }
        
        Ok(statuses)
    }
    
    async fn mark_sensor_readings_uploaded(&self, ids: &[ReadingId]) -> Result<(), Self::Error> {
        if ids.is_empty() {
            return Ok(());
        }
        
        let conn = self.conn.lock().await;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e)))?;
        
        for id in ids {
            let id_str = id.0.to_string();
            tx.execute(
                "UPDATE sensor_readings SET state = 'uploaded' WHERE id = ?",
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
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Transaction failed: {}", e)))?;
        
        for id in ids {
            let id_str = id.0.to_string();
            tx.execute(
                "UPDATE device_statuses SET state = 'uploaded' WHERE id = ?",
                params![id_str],
            )
            .map_err(|e| SqliteStorageError::UpdateFailed(format!("Update failed for {}: {}", id_str, e)))?;
        }
        
        tx.commit()
            .map_err(|e| SqliteStorageError::TransactionFailed(format!("Commit failed: {}", e)))?;
        
        Ok(())
    }
}
