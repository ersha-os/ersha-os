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
