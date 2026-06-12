mod clock;

mod errors;
mod identity;
mod pagination;
mod resilience;
mod transaction;

pub use clock::{Clock, SystemClock};
pub use errors::{Error, ErrorCode, Result};
pub use identity::{
    Entity, EntityOptionExt, Identifier, LifecycleTracker, ManagedEntity, ValueObject, Versioned,
};
pub use pagination::{PageQuery, PagedResult};
pub use resilience::{RetryConfig, with_retry};
pub use transaction::{Transaction, TransactionManager, TransactionManagerExt};

#[cfg(feature = "concurrency")]
mod concurrency;

#[cfg(feature = "concurrency")]
pub use concurrency::Singleflight;
