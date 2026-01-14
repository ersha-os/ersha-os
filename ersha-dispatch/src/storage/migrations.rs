use sqlx::{Error as SqlxError, SqlitePool};

#[derive(Debug)]
pub enum MigrationError {
    Sqlx(SqlxError),
    VersionError(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Sqlx(e) => write!(f, "SQLx error: {}", e),
            MigrationError::VersionError(e) => write!(f, "Version error: {}", e),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<SqlxError> for MigrationError {
    fn from(err: SqlxError) -> Self {
        MigrationError::Sqlx(err)
    }
}

pub struct Migrator;

impl Migrator {
    /// run all pending migrations safely
    pub async fn run_migrations(pool: &SqlitePool) -> Result<(), MigrationError> {
        // create version table if not exists
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(pool)
        .await?;

        let current_version = Self::get_version(pool).await?;

        for migration in MIGRATIONS.iter() {
            if migration.version > current_version {
                // start transaction for atomic migration
                let mut tx = pool.begin().await?;

                // run migration SQL
                sqlx::query(migration.sql).execute(&mut *tx).await?;

                // update version
                sqlx::query("INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?)")
                    .bind(migration.version)
                    .execute(&mut *tx)
                    .await?;

                tx.commit().await?;
            }
        }

        Ok(())
    }

    /// get current database version
    pub async fn get_version(pool: &SqlitePool) -> Result<i64, MigrationError> {
        let result: Option<(i64,)> =
            sqlx::query_as("SELECT COALESCE(version, 0) FROM schema_version WHERE id = 1")
                .fetch_optional(pool)
                .await?;

        Ok(result.map(|(v,)| v).unwrap_or(0))
    }

    pub async fn column_exists(
        pool: &SqlitePool,
        table: &str,
        column: &str,
    ) -> Result<bool, SqlxError> {
        let result: Option<(i64,)> =
            sqlx::query_as("SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?")
                .bind(table)
                .bind(column)
                .fetch_optional(pool)
                .await?;

        Ok(result.map(|(count,)| count > 0).unwrap_or(false))
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
