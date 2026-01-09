use std::collections::HashMap;

use ersha_core::{Dispatcher, DispatcherId, DispatcherState};

use super::{
    DispatcherRegistry,
    filter::{DispatcherFilter, DispatcherSortBy, QueryOptions},
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
        todo!()
    }

    async fn list(
        &self,
        options: QueryOptions<DispatcherFilter, DispatcherSortBy>,
    ) -> Result<Vec<Dispatcher>, Self::Error> {
        todo!()
    }
}
