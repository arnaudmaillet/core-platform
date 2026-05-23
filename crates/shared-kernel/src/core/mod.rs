mod clock;

mod errors;
mod identity;
mod resilience;
mod transaction;

pub use clock::{Clock, SystemClock};
pub use errors::{Error, ErrorCode, Result};
pub use identity::{
    AggregateMetadata, AggregateRoot, Entity, EntityOptionExt, Identifier, ValueObject, Versioned,
};
pub use resilience::{RetryConfig, with_retry};
pub use transaction::{Transaction, TransactionManager};

#[cfg(feature = "stub")]
pub use transaction::{TransactionManagerStub, TransactionStub};

#[cfg(feature = "concurrency")]
mod concurrency;

#[cfg(feature = "concurrency")]
pub use concurrency::Singleflight;
