use rusqlite::{params, Connection};

#[derive(Debug)]
pub enum MigrationError {
    Sqlite(rusqlite::Error),
    VersionError(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Sqlite(e) => write!(f, "SQLite error: {}", e),
            MigrationError::VersionError(e) => write!(f, "Version error: {}", e),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(err: rusqlite::Error) -> Self {
        MigrationError::Sqlite(err)
    }
}

pub struct Migrator;

impl Migrator {
    /// Run all pending migrations safely
    pub fn run_migrations(conn: &Connection) -> Result<(), MigrationError> {
        // Create version table if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        let current_version: i64 = conn
            .query_row(
                "SELECT COALESCE(version, 0) FROM schema_version WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        for migration in MIGRATIONS.iter() {
            if migration.version > current_version {
                // Start transaction for atomic migration
                let tx = conn.unchecked_transaction()?;

                // Run migration SQL
                tx.execute_batch(migration.sql)?;

                // Update version
                tx.execute(
                    "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?)",
                    params![migration.version],
                )?;

                tx.commit()?;
            }
        }

        Ok(())
    }

    /// Get current database version
    pub fn get_version(conn: &Connection) -> Result<i64, MigrationError> {
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(version, 0) FROM schema_version WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(version)
    }

    pub fn column_exists(
        conn: &Connection,
        table: &str,
        column: &str,
    ) -> Result<bool, rusqlite::Error> {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?",
            params![table, column],
            |row| row.get(0),
        )?;
        Ok(exists > 0)
    }
}

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: r#"
            -- Core tables for agricultural data
            CREATE TABLE IF NOT EXISTS sensor_readings (
                id TEXT PRIMARY KEY,
                reading_json TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                uploaded_at TIMESTAMP
            );
            
            CREATE TABLE IF NOT EXISTS device_statuses (
                id TEXT PRIMARY KEY,
                status_json TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('pending', 'uploaded')),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                uploaded_at TIMESTAMP
            );
            
            -- Indexes for common queries
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_state 
            ON sensor_readings(state);
            
            CREATE INDEX IF NOT EXISTS idx_device_statuses_state 
            ON device_statuses(state);
            
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_created_at 
            ON sensor_readings(created_at);
            
            CREATE INDEX IF NOT EXISTS idx_device_statuses_created_at 
            ON device_statuses(created_at);
            
            CREATE INDEX IF NOT EXISTS idx_sensor_readings_uploaded_at 
            ON sensor_readings(uploaded_at);
            
            CREATE INDEX IF NOT EXISTS idx_device_statuses_uploaded_at 
            ON device_statuses(uploaded_at);
        "#,
}];
