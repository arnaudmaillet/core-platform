mod repository;
pub use repository::IdempotencyRepository;

#[cfg(feature = "stub")]
mod repository_stub;

#[cfg(feature = "stub")]
pub use repository_stub::IdempotencyRepositoryStub;
