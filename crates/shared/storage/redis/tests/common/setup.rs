use std::sync::OnceLock;

use redis_storage::{RedisClient, RedisClientBuilder, RedisConfig, RedisPool, RedisPoolBuilder};

static TRACING: OnceLock<()> = OnceLock::new();

/// Initialises a `tracing-subscriber` for test output exactly once per
/// process. Safe to call from every test function; subsequent calls are
/// no-ops.
pub fn init_tracing() {
    TRACING.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("redis_storage=debug,fred=info")
            .try_init()
            .ok();
    });
}

/// Reads `REDIS_HOSTS` from the environment and builds a standalone
/// [`RedisClient`] for integration tests.
///
/// Falls back to `127.0.0.1:6379` when `REDIS_HOSTS` is unset.
///
/// Mark every test that calls this with `#[ignore]` so that CI skips it
/// unless a live Redis instance is available. Run with:
///
/// ```sh
/// cargo test -p redis-storage -- --include-ignored
/// # or
/// REDIS_HOSTS=127.0.0.1:6379 cargo test -p redis-storage -- --include-ignored
/// ```
#[allow(dead_code)]
pub async fn test_client() -> RedisClient {
    init_tracing();
    let config = RedisConfig::from_env();
    RedisClientBuilder::new(config)
        .build()
        .await
        .expect("integration test: failed to build RedisClient")
}

/// Builds a [`RedisPool`] for integration tests.
///
/// Uses `REDIS_POOL_SIZE` (default: `2`) to keep test resource usage low.
/// Mark every test that calls this with `#[ignore]`.
#[allow(dead_code)]
pub async fn test_pool() -> RedisPool {
    init_tracing();
    let mut config = RedisConfig::from_env();
    config.pool_size = std::env::var("REDIS_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    RedisPoolBuilder::new(config)
        .build()
        .await
        .expect("integration test: failed to build RedisPool")
}
