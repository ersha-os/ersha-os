mod device;
mod dispatcher;

#[derive(Debug)]
pub enum SqliteRegistryError {
    Sqlx(sqlx::Error),
    InvalidUlid(String),
    InvalidTimestamp(i64),
    InvalidState(i32),
    NotFound,
}

impl From<sqlx::Error> for SqliteRegistryError {
    fn from(e: sqlx::Error) -> Self {
        Self::Sqlx(e)
    }
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use ulid::Ulid;

    use crate::registry::DispatcherRegistry;
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };
    use crate::registry::sqlite::dispatcher::SqliteDispatcherRegistry;
    use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};

    use sqlx::SqlitePool;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::SqlitePoolOptions;

    static MIGRATROR: Migrator = sqlx::migrate!("./migrations");

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create pool");

        MIGRATROR
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

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
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let id = DispatcherId(Ulid::new());
        let dispatcher = dispatcher(id, DispatcherState::Active, Timestamp::now());

        registry.register(dispatcher).await.expect("Save failed");
        let fetched = registry.get(id).await.expect("Query failed");

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_sqlite_list_with_filters() {
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

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
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

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
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

        let ids = vec![Ulid::new(), Ulid::new(), Ulid::new()];
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
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };

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
        let pool = setup_db().await;
        let mut registry = SqliteDispatcherRegistry { pool };
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
