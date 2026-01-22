use std::str::FromStr;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, migrate::Migrator, sqlite::SqlitePoolOptions};
use ulid::Ulid;

use async_trait::async_trait;

use crate::registry::{
    DispatcherRegistry,
    filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum SqliteDispatcherError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("invalid dispatcher state: {0}")]
    InvalidState(i32),
    #[error("not found")]
    NotFound,
}

pub struct SqliteDispatcherRegistry {
    pool: SqlitePool,
}

impl SqliteDispatcherRegistry {
    pub async fn new(path: impl AsRef<str>) -> Result<Self, SqliteDispatcherError> {
        let connection_string = format!("sqlite:{}", path.as_ref());
        let pool = SqlitePoolOptions::new().connect(&connection_string).await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn new_in_memory() -> Result<Self, SqliteDispatcherError> {
        let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl DispatcherRegistry for SqliteDispatcherRegistry {
    type Error = SqliteDispatcherError;

    async fn register(&self, dispatcher: Dispatcher) -> Result<(), Self::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO dispatchers (id, state, location, provisioned_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(dispatcher.id.0.to_string())
        .bind(dispatcher.state as i32)
        .bind(dispatcher.location.0 as i64)
        .bind(dispatcher.provisioned_at.as_second())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, state, location, provisioned_at FROM dispatchers WHERE id = ?
            "#,
        )
        .bind(id.0.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| -> Result<Dispatcher, SqliteDispatcherError> {
            let id = r.try_get::<String, _>("id")?;
            let ulid = Ulid::from_str(&id)
                .map_err(|_| SqliteDispatcherError::InvalidUlid(id.to_string()))?;

            let provisioned_at = r.try_get::<i64, _>("provisioned_at")?;
            let provisioned_at = jiff::Timestamp::from_second(provisioned_at)
                .map_err(|_| SqliteDispatcherError::InvalidTimestamp(provisioned_at))?;

            let state = match r.try_get::<i32, _>("state")? {
                0 => DispatcherState::Active,
                1 => DispatcherState::Suspended,
                other => return Err(SqliteDispatcherError::InvalidState(other)),
            };

            Ok(Dispatcher {
                id: DispatcherId(ulid),
                location: H3Cell(r.try_get::<i64, _>("location")? as u64),
                state,
                provisioned_at,
            })
        })
        .transpose()
    }

    async fn update(&self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error> {
        let old = self.get(id).await?.ok_or(SqliteDispatcherError::NotFound)?;
        let new = Dispatcher { id: old.id, ..new };

        self.register(new).await
    }

    async fn suspend(&self, id: DispatcherId) -> Result<(), Self::Error> {
        let dispatcher = self.get(id).await?.ok_or(SqliteDispatcherError::NotFound)?;

        let new = Dispatcher {
            state: DispatcherState::Suspended,
            ..dispatcher
        };

        self.register(new).await
    }

    async fn batch_register(&self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error> {
        let mut tx = self.pool.begin().await?;

        for dispatcher in dispatchers {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO dispatchers (id, state, location, provisioned_at)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(dispatcher.id.0.to_string())
            .bind(dispatcher.state as i32)
            .bind(dispatcher.location.0 as i64)
            .bind(dispatcher.provisioned_at.as_second())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error> {
        let mut query_builder = QueryBuilder::new("SELECT COUNT(*) FROM dispatchers ");

        if let Some(filter) = filter {
            query_builder = filter_dispatchers(query_builder, filter);
        }

        let query = query_builder.build();
        let count: i64 = query.fetch_one(&self.pool).await?.try_get(0)?;

        Ok(count as usize)
    }

    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<ersha_core::Dispatcher>, Self::Error> {
        let mut query_builder =
            QueryBuilder::new("SELECT id, state, location, provisioned_at FROM dispatchers");

        query_builder = filter_dispatchers(query_builder, options.filter);

        match options.sort_by {
            DispatcherSortBy::ProvisionAt => query_builder.push(" ORDER BY provisioned_at"),
        };

        match options.sort_order {
            SortOrder::Asc => query_builder.push(" ASC "),
            SortOrder::Desc => query_builder.push(" DESC "),
        };

        if let Pagination::Offset { offset, limit } = options.pagination {
            query_builder.push(" LIMIT ");
            query_builder.push_bind(limit as i64);

            query_builder.push(" OFFSET ");
            query_builder.push_bind(offset as i64);
        }

        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        rows.into_iter()
            .map(|r| {
                let id = r.try_get::<String, _>("id")?;
                let ulid =
                    Ulid::from_str(&id).map_err(|_| SqliteDispatcherError::InvalidUlid(id))?;

                let provisioned_at = r.try_get::<i64, _>("provisioned_at")?;
                let provisioned_at = jiff::Timestamp::from_second(provisioned_at)
                    .map_err(|_| SqliteDispatcherError::InvalidTimestamp(provisioned_at))?;

                let state = match r.try_get::<i32, _>("state")? {
                    0 => DispatcherState::Active,
                    1 => DispatcherState::Suspended,
                    other => return Err(SqliteDispatcherError::InvalidState(other)),
                };

                Ok(Dispatcher {
                    id: DispatcherId(ulid),
                    provisioned_at,
                    state,
                    location: H3Cell(r.try_get::<i64, _>("location")? as u64),
                })
            })
            .collect()
    }
}

fn filter_dispatchers(
    mut query_builder: QueryBuilder<Sqlite>,
    filter: DispatcherFilter,
) -> QueryBuilder<Sqlite> {
    let mut has_where = false;

    if let Some(states) = filter.states
        && !states.is_empty()
    {
        query_builder.push(" WHERE state IN (");
        let mut separated = query_builder.separated(", ");
        for state in states {
            separated.push_bind(state as i32);
        }
        separated.push_unseparated(")");
        has_where = true;
    }

    if let Some(locations) = filter.locations
        && !locations.is_empty()
    {
        if has_where {
            query_builder.push(" AND ");
        } else {
            query_builder.push(" WHERE ");
        }

        query_builder.push("location IN (");

        let mut separated = query_builder.separated(", ");
        for location in locations {
            separated.push_bind(location.0 as i64);
        }

        separated.push_unseparated(")");
    }

    query_builder
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use ulid::Ulid;

    use crate::registry::DispatcherRegistry;
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};

    use super::SqliteDispatcherRegistry;

    fn dispatcher(
        id: DispatcherId,
        state: DispatcherState,
        provisioned_at: Timestamp,
    ) -> Dispatcher {
        Dispatcher {
            id,
            state,
            location: H3Cell(0x1337deadbeef),
            provisioned_at,
        }
    }

    fn default_options() -> QueryOptions<DispatcherFilter, DispatcherSortBy> {
        QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 100,
            },
        }
    }

    #[tokio::test]
    async fn test_sqlite_register_and_get() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();

        let id = DispatcherId(Ulid::new());
        let dispatcher = dispatcher(id, DispatcherState::Active, Timestamp::now());

        registry.register(dispatcher).await.expect("Save failed");
        let fetched = registry.get(id).await.expect("Query failed");

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_sqlite_list_with_filters() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();

        let id1 = DispatcherId(Ulid::new());
        let d1 = Dispatcher {
            id: id1,
            state: DispatcherState::Active,
            location: H3Cell(1),
            provisioned_at: Timestamp::now(),
        };
        registry.register(d1).await.unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: Some(vec![DispatcherState::Suspended]),
                ..Default::default()
            },
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_empty_filter_fields() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();

        let id = DispatcherId(Ulid::new());
        registry
            .register(dispatcher(id, DispatcherState::Active, Timestamp::now()))
            .await
            .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: None,
                locations: None,
            },
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 10,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should ignore empty filters and return all records"
        );
    }

    #[tokio::test]
    async fn test_sqlite_multiple_state_filter() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();

        let ids = [Ulid::new(), Ulid::new(), Ulid::new()];
        registry
            .batch_register(vec![
                dispatcher(
                    DispatcherId(ids[0]),
                    DispatcherState::Active,
                    Timestamp::now(),
                ),
                dispatcher(
                    DispatcherId(ids[1]),
                    DispatcherState::Suspended,
                    Timestamp::now(),
                ),
                dispatcher(
                    DispatcherId(ids[2]),
                    DispatcherState::Suspended,
                    Timestamp::now(),
                ),
            ])
            .await
            .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter {
                states: Some(vec![DispatcherState::Active]),
                ..Default::default()
            },
            ..default_options()
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results
                .iter()
                .all(|d| d.state != DispatcherState::Suspended)
        );
    }

    #[tokio::test]
    async fn test_sqlite_pagination_offset_logic() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();

        for i in 0..5 {
            let d = dispatcher(
                DispatcherId(Ulid::new()),
                DispatcherState::Active,
                Timestamp::from_second(i).unwrap(),
            );
            registry.register(d).await.unwrap();
        }

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Offset {
                offset: 2,
                limit: 2,
            },
        };

        let results = registry.list(options).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].provisioned_at,
            Timestamp::from_second(2).unwrap()
        );
        assert_eq!(
            results[1].provisioned_at,
            Timestamp::from_second(3).unwrap()
        );
    }

    #[tokio::test]
    async fn test_sqlite_count_after_suspend() {
        let registry = SqliteDispatcherRegistry::new_in_memory().await.unwrap();
        let id = DispatcherId(Ulid::new());

        registry
            .register(dispatcher(id, DispatcherState::Active, Timestamp::now()))
            .await
            .unwrap();

        let active_filter = DispatcherFilter {
            states: Some(vec![DispatcherState::Active]),
            ..Default::default()
        };

        assert_eq!(
            registry.count(Some(active_filter.clone())).await.unwrap(),
            1
        );

        registry.suspend(id).await.unwrap();

        assert_eq!(registry.count(Some(active_filter)).await.unwrap(), 0);
    }
}
