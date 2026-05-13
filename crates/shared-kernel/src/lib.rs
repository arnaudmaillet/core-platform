// crates/shared-kernel/src/lib.rs

pub mod application;
mod building_blocks;
pub mod core;
mod persistence;
pub mod test_utils;
mod transport;

pub use application::{idempotency, sharding};
pub use building_blocks::{geo, messaging, security, types};
pub use persistence::{cache, postgres, redis, scylla};

#[cfg(feature = "kafka")]
pub use transport::kafka;
