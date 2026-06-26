//! Redis adapters — the hot-path delivery cache.
//!
//! Keys carry a `{…}` hash tag on the asset id so any future multi-key operation
//! stays slot-safe on Redis Cluster (the mandatory slot-safety rule).

pub mod keys;
pub mod redis_delivery_cache;

pub use redis_delivery_cache::RedisDeliveryCache;
