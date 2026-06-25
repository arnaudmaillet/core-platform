//! Redis adapters — the hot-path enforcement projection (Plane B) and the
//! known-bad screen corpus (Plane C).

pub mod keys;
pub mod redis_enforcement_projection;
pub mod redis_screen_corpus;

pub use redis_enforcement_projection::RedisEnforcementProjection;
pub use redis_screen_corpus::RedisScreenCorpus;
