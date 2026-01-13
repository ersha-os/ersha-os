use std::str::FromStr;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use ulid::Ulid;

use crate::registry::{
    filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder},
    DispatcherRegistry,
};

#[derive(Debug)]
pub enum SqliteDispatcherError {
    Sqlx(sqlx::Error),
    InvalidUlid(String),
    InvalidTimestamp(i64),
    InvalidState(i32),
    NotFound,
}

impl From<sqlx::Error> for SqliteDispatcherError {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}

pub struct SqliteDispatcherRegistry {
    pub pool: SqlitePool,
}

impl SqliteDispatcherRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl DispatcherRegistry for SqliteDispatcherRegistry {
    type Error = SqliteDispatcherError;

    async fn register(&mut self, dispatcher: Dispatcher) -> Result<(), Self::Error> {
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

    async fn update(&mut self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error> {
        let old = self.get(id).await?.ok_or(SqliteDispatcherError::NotFound)?;
        let new = Dispatcher { id: old.id, ..new };

        self.register(new).await
    }

    async fn suspend(&mut self, id: DispatcherId) -> Result<(), Self::Error> {
        let dispatcher = self.get(id).await?.ok_or(SqliteDispatcherError::NotFound)?;

        let new = Dispatcher {
            state: DispatcherState::Suspended,
            ..dispatcher
        };

        self.register(new).await
    }

    async fn batch_register(&mut self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error> {
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
