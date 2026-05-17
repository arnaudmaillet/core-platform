// crates/shared-kernel/src/lib.rs

pub mod application;
mod building_blocks;
pub mod core;
mod persistence;
mod transport;

pub use application::{command, context, idempotency, sharding};
pub use building_blocks::{geo, messaging, security, types};
pub use persistence::cache;

#[cfg(feature = "postgres")]
pub use persistence::postgres;

#[cfg(feature = "scylla")]
pub use persistence::scylla;

#[cfg(feature = "redis")]
pub use persistence::redis;

#[cfg(feature = "kafka")]
pub use transport::kafka;

#[cfg(feature = "test-utils")]
pub mod test_utils;
