use std::collections::HashMap;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState};

use super::{
    DispatcherRegistry,
    filter::{DispatcherFilter, DispatcherSortBy, Pagination, QueryOptions, SortOrder},
};

pub enum InMemoryError {
    NotFound,
}

pub struct InMemoryDispatcherRegistry {
    dispatchers: HashMap<DispatcherId, Dispatcher>,
}

impl DispatcherRegistry for InMemoryDispatcherRegistry {
    type Error = InMemoryError;

    async fn register(&mut self, dispatcher: Dispatcher) -> Result<(), Self::Error> {
        let _ = self.dispatchers.insert(dispatcher.id, dispatcher);
        Ok(())
    }

    async fn get(&self, id: DispatcherId) -> Result<Option<Dispatcher>, Self::Error> {
        Ok(self.dispatchers.get(&id).cloned())
    }

    async fn update(&mut self, id: DispatcherId, new: Dispatcher) -> Result<(), Self::Error> {
        let _old = self.dispatchers.insert(id, new);
        Ok(())
    }

    async fn suspend(&mut self, id: DispatcherId) -> Result<(), Self::Error> {
        let dispatcher = self.get(id).await?.ok_or(InMemoryError::NotFound)?;

        let _ = self
            .update(
                id,
                Dispatcher {
                    state: DispatcherState::Suspended,
                    ..dispatcher
                },
            )
            .await?;

        Ok(())
    }

    async fn batch_register(&mut self, dispatchers: Vec<Dispatcher>) -> Result<(), Self::Error> {
        for dispatcher in dispatchers {
            self.register(dispatcher).await?;
        }

        Ok(())
    }

    async fn count(&self, filter: Option<DispatcherFilter>) -> Result<usize, Self::Error> {
        if let Some(filter) = filter {
            let filtered = filter_dispatchers(&self.dispatchers, &filter);

            return Ok(filtered.count());
        }

        Ok(self.dispatchers.len())
    }

    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error> {
        let filtered: Vec<&Dispatcher> =
            filter_dispatchers(&self.dispatchers, &options.filter).collect();
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
                let id = DispatcherId(inner_ulid.clone());
                return dispatchers
                    .into_iter()
                    .skip_while(|dispatcher| dispatcher.id != id)
                    .skip(1)
                    .take(*limit)
                    .cloned()
                    .collect();
            }

            return vec![];
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
    dispatchers.values().filter_map(|dispatcher| {
        if let Some(locations) = &filter.locations {
            if !locations.contains(&dispatcher.location) {
                return None;
            }
        }

        if let Some(states) = &filter.states {
            if !states.contains(&dispatcher.state) {
                return None;
            }
        }

        return Some(dispatcher);
    })
}
