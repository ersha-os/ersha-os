mod device;
mod dispatcher;

#[derive(Debug)]
pub enum InMemoryError {
    NotFound,
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use std::collections::HashMap;
    use ulid::Ulid;

    use crate::registry::DispatcherRegistry;
    use crate::registry::filter::{
        DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder,
    };
    use ersha_core::{Dispatcher, DispatcherId, DispatcherState, H3Cell};

    use super::dispatcher::InMemoryDispatcherRegistry;

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

    fn registry() -> InMemoryDispatcherRegistry {
        InMemoryDispatcherRegistry {
            dispatchers: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let mut reg = registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d.clone()).await.unwrap();
        let fetched = reg.get(id).await.unwrap().expect("Dispatcher should exist");

        assert_eq!(fetched.id, id);
        assert_eq!(fetched.state, DispatcherState::Active);
    }

    #[tokio::test]
    async fn test_suspend_logic() {
        let mut reg = registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d).await.unwrap();
        reg.suspend(id).await.unwrap();

        let updated = reg.get(id).await.unwrap().unwrap();
        assert_eq!(updated.state, DispatcherState::Suspended);
    }

    #[tokio::test]
    async fn test_count_with_filter() {
        let mut reg = registry();
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());

        reg.batch_register(vec![
            dispatcher(id1, DispatcherState::Active, Timestamp::now()),
            dispatcher(id2, DispatcherState::Suspended, Timestamp::now()),
        ])
        .await
        .unwrap();

        assert_eq!(reg.count(None).await.unwrap(), 2);

        let filter = DispatcherFilter {
            states: Some(vec![DispatcherState::Active]),
            ..Default::default()
        };
        assert_eq!(reg.count(Some(filter)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_sorting_and_pagination() {
        let mut reg = registry();

        // Create 3 dispatchers with distinct timestamps
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());
        let id3 = DispatcherId(Ulid::new());

        reg.batch_register(vec![
            dispatcher(
                id1,
                DispatcherState::Active,
                Timestamp::from_second(100).unwrap(),
            ),
            dispatcher(
                id2,
                DispatcherState::Active,
                Timestamp::from_second(300).unwrap(),
            ),
            dispatcher(
                id3,
                DispatcherState::Active,
                Timestamp::from_second(200).unwrap(),
            ),
        ])
        .await
        .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Desc,
            pagination: Pagination::Offset {
                offset: 0,
                limit: 2,
            },
        };

        let results = reg.list(options).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, id2);
        assert_eq!(results[1].id, id3);
    }

    #[tokio::test]
    async fn test_cursor_pagination() {
        let mut reg = registry();
        let id1 = DispatcherId(Ulid::new());
        let id2 = DispatcherId(Ulid::new());

        // Important for cursor: In-memory hashmap order is random,
        // but our sort_dispatchers makes it deterministic
        reg.batch_register(vec![
            dispatcher(
                id1,
                DispatcherState::Active,
                Timestamp::from_second(10).unwrap(),
            ),
            dispatcher(
                id2,
                DispatcherState::Active,
                Timestamp::from_second(20).unwrap(),
            ),
        ])
        .await
        .unwrap();

        let options = QueryOptions {
            filter: DispatcherFilter::default(),
            sort_by: DispatcherSortBy::ProvisionAt,
            sort_order: SortOrder::Asc,
            pagination: Pagination::Cursor {
                after: Some(id1.0),
                limit: 1,
            },
        };

        let results = reg.list(options).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id2);
    }
}
