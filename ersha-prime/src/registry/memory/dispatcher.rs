use std::collections::HashMap;

use async_trait::async_trait;
use ersha_core::{Dispatcher, DispatcherId, DispatcherState};
use tokio::sync::RwLock;

use crate::registry::{
    DispatcherRegistry,
    filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder},
};

use super::InMemoryError;

pub struct InMemoryDispatcherRegistry {
    pub dispatchers: RwLock<HashMap<DispatcherId, Dispatcher>>,
}

#[async_trait]
impl DispatcherRegistry for InMemoryDispatcherRegistry {
    type Error = InMemoryError;

    async fn register(&self, dispatcher: Dispatcher) -> Result<(), Self::Error> {
        let mut dispatchers = self.dispatchers.write().await;
        let _ = dispatchers.insert(dispatcher.id, dispatcher);
        Ok(())
    }

    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error> {
        let dispatchers = self.dispatchers.read().await;
        Ok(dispatchers.get(&id).cloned())
    }

    async fn update(&self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error> {
        let mut dispatchers = self.dispatchers.write().await;
        let _old = dispatchers.insert(id, new);
        Ok(())
    }

    async fn suspend(&self, id: DispatcherId) -> Result<(), Self::Error> {
        let dispatcher = self.get(id).await?.ok_or(InMemoryError::NotFound)?;

        self.update(
            id,
            Dispatcher {
                state: DispatcherState::Suspended,
                ..dispatcher
            },
        )
        .await?;

        Ok(())
    }

    async fn batch_register(&self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error> {
        for dispatcher in dispatchers {
            self.register(dispatcher).await?;
        }

        Ok(())
    }

    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error> {
        let dispatchers = self.dispatchers.read().await;
        if let Some(filter) = filter {
            let filtered = filter_dispatchers(&dispatchers, &filter);

            return Ok(filtered.count());
        }

        Ok(dispatchers.len())
    }

    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error> {
        let dispatchers = self.dispatchers.read().await;
        let filtered: Vec<&Dispatcher> =
            filter_dispatchers(&dispatchers, &options.filter).collect();
        let sorted = sort_dispatchers(filtered, &options.sort_by, &options.sort_order);
        let paginated = paginate_dispatchers(sorted, &options.pagination);

        Ok(paginated)
    }
}

fn paginate_dispatchers(dispatchers: Vec<&Dispatcher>, pagination: &Pagination) -> Vec<Dispatcher> {
    match pagination {
        Pagination::Offset { offset, limit } => dispatchers
            .into_iter()
            .skip(*offset)
            .take(*limit)
            .cloned()
            .collect(),
        Pagination::Cursor { after, limit } => {
            if let Some(inner_ulid) = after {
                let id = DispatcherId(*inner_ulid);
                return dispatchers
                    .into_iter()
                    .skip_while(|dispatcher| dispatcher.id != id)
                    .skip(1)
                    .take(*limit)
                    .cloned()
                    .collect();
            }

            vec![]
        }
    }
}

fn sort_dispatchers<'a>(
    mut dispatchers: Vec<&'a Dispatcher>,
    sort_by: &DispatcherSortBy,
    sort_order: &SortOrder,
) -> Vec<&'a Dispatcher> {
    dispatchers.sort_by(|a, b| {
        let ord = match sort_by {
            DispatcherSortBy::ProvisionAt => a.provisioned_at.cmp(&b.provisioned_at),
        };

        match sort_order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        }
    });

    dispatchers
}

fn filter_dispatchers<'a>(
    dispatchers: &'a HashMap<DispatcherId, Dispatcher>,
    filter: &DispatcherFilter,
) -> impl Iterator<Item = &'a Dispatcher> {
    dispatchers.values().filter(|dispatcher| {
        if let Some(locations) = &filter.locations
            && !locations.contains(&dispatcher.location)
        {
            return false;
        }

        if let Some(states) = &filter.states
            && !states.contains(&dispatcher.state)
        {
            return false;
        }

        true
    })
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

    use super::InMemoryDispatcherRegistry;

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

    fn dispatcher_registry() -> InMemoryDispatcherRegistry {
        InMemoryDispatcherRegistry {
            dispatchers: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let reg = dispatcher_registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d.clone()).await.unwrap();
        let fetched = reg.get(id).await.unwrap().expect("Dispatcher should exist");

        assert_eq!(fetched.id, id);
        assert_eq!(fetched.state, DispatcherState::Active);
    }

    #[tokio::test]
    async fn test_suspend_logic() {
        let reg = dispatcher_registry();
        let id = DispatcherId(Ulid::new());
        let d = dispatcher(id, DispatcherState::Active, Timestamp::now());

        reg.register(d).await.unwrap();
        reg.suspend(id).await.unwrap();

        let updated = reg.get(id).await.unwrap().unwrap();
        assert_eq!(updated.state, DispatcherState::Suspended);
    }

    #[tokio::test]
    async fn test_count_with_filter() {
        let reg = dispatcher_registry();
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
        let reg = dispatcher_registry();

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
        let reg = dispatcher_registry();
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
