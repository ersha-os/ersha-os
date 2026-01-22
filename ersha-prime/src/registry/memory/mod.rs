mod device;
mod dispatcher;

#[derive(Debug, thiserror::Error)]
pub enum InMemoryError {
    #[error("not found")]
    NotFound,
}
