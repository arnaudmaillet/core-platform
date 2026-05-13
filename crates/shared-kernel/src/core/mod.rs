mod clock;

mod errors;
mod identity;
mod resilience;
mod transaction;

pub use clock::Clock;

pub use errors::{Error, ErrorCode, Result};
pub use identity::{AggregateMetadata, AggregateRoot, Entity, Identifier, ValueObject, Versioned};
pub use resilience::{RetryConfig, with_retry};
pub use transaction::{FakeTransaction, Transaction, TransactionManager};

#[cfg(feature = "concurrency")]
mod concurrency;

#[cfg(feature = "concurrency")]
pub use concurrency::Singleflight;
