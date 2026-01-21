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

#[cfg(feature = "scylla")]
pub mod scylla;

pub mod grpc;



