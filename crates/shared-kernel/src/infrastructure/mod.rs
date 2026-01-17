// crates/shared-kernel/src/infrastructure/mod.rs

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "kafka")]
pub mod kafka;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(any(feature = "redis", feature = "postgres", feature = "kafka"))]
pub mod bootstrap;

#[cfg(feature = "concurrency")]
pub mod concurrency;

// Utilitaires de base (souvent sans drivers lourds)
#[cfg(feature = "postgres")]
mod transaction;


#[cfg(feature = "runtime")]
mod retry;

mod pagination;
mod repository;

#[cfg(feature = "postgres")]
mod transaction_manager;
#[cfg(all(feature = "postgres", feature = "kafka"))]
mod outbox_processor;

#[cfg(feature = "postgres")]
pub use transaction::TransactionExt;

pub use pagination::{PageResponse, PageRequest};
pub use repository::BaseRepository;

#[cfg(feature = "runtime")]
pub use retry::{RetryConfig, with_retry};

#[cfg(feature = "postgres")]
pub use transaction_manager::TransactionManagerExt;
#[cfg(all(feature = "postgres", feature = "kafka"))]
pub use outbox_processor::OutboxProcessor;