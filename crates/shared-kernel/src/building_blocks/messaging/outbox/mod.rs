mod processor;
mod processor_test;
mod repository;
mod store;

pub use processor::OutboxProcessor;
pub use repository::OutboxRepository;
pub use store::OutboxStore;

#[cfg(feature = "test-utils")]
pub mod repository_stub;

#[cfg(feature = "test-utils")]
pub use repository_stub::OutboxRepositoryStub;

#[cfg(all(feature = "postgres", feature = "kafka"))]
mod relay;
#[cfg(all(feature = "postgres", feature = "kafka"))]
pub use relay::run_outbox_relay;
