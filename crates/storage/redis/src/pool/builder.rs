use std::ops::Deref;

use fred::clients::Pool as FredPool;
use fred::interfaces::ClientLike as _;

use crate::config::connection::RedisConfig;
use crate::error::map::RedisStorageError;
use crate::listener::event::spawn_event_listener;

/// A fixed-size pool of multiplexed Redis connections.
///
/// `RedisPool` wraps `fred::clients::Pool`, which round-robins commands across
/// `N` independent `Client` instances. Use this when a single multiplexed
/// connection (see [`crate::client::builder::RedisClient`]) becomes the
/// throughput bottleneck — typically in services executing large bulk pipelines,
/// high-fan-out pub/sub publishing, or sustained > 100k cmd/s per node.
///
/// ## Pool size guidance
///
/// Start with `REDIS_POOL_SIZE=8`. Profile at production load before
/// increasing — fred's multiplexer is highly efficient, and more connections
/// increase server-side memory and file-descriptor pressure without benefit
/// once the bottleneck shifts to the network or Redis CPU.
///
/// ## Cloning
///
/// `RedisPool` is cheaply cloneable — `fred::clients::Pool` is backed by an
/// `Arc`. Pass clones across CQRS handlers or Axum state without locking.
///
/// ## Usage
///
/// ```rust,ignore
/// use fred::interfaces::{ClientLike, KeysInterface};
/// use redis_storage::{RedisPoolBuilder, RedisConfig};
///
/// let config = RedisConfig::from_env();
/// let pool = RedisPoolBuilder::new(config).build().await?;
///
/// pool.set("timeline:user:42", serialized, None, None, false).await?;
/// ```
#[derive(Clone)]
pub struct RedisPool {
    /// The underlying fred connection pool.
    ///
    /// Import `fred::interfaces::ClientLike` (and other command traits) to
    /// issue Redis commands directly. The pool round-robins across its
    /// member connections automatically.
    pub inner: FredPool,
}

impl Deref for RedisPool {
    type Target = FredPool;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Orchestrates `RedisPool` construction from a [`RedisConfig`].
///
/// Performs the following steps in order:
/// 1. Converts [`RedisConfig`] into a fred `Builder` (topology + tuning).
/// 2. Calls `Builder::build_pool(pool_size)` to instantiate all pool members.
/// 3. Calls `pool.init().await` to connect all members concurrently and block
///    until every connection is established (subject to `fail_fast` and timeout).
/// 4. Spawns the OTel event listener on the pool (see [`spawn_event_listener`]).
///
/// ## Errors
///
/// Returns [`RedisStorageError::Configuration`] when the config is invalid.
/// Returns [`RedisStorageError::Disconnected`] when any pool member fails to
/// connect within the configured timeout and `fail_fast` is `true`.
pub struct RedisPoolBuilder {
    config: RedisConfig,
}

impl RedisPoolBuilder {
    pub fn new(config: RedisConfig) -> Self {
        Self { config }
    }

    /// Connects all pool members and returns a [`RedisPool`] ready for use.
    ///
    /// # Errors
    ///
    /// See [`RedisPoolBuilder`] error documentation.
    #[tracing::instrument(
        name  = "redis.pool.build",
        skip(self),
        fields(topology = ?self.config.topology, pool_size = self.config.pool_size)
    )]
    pub async fn build(self) -> Result<RedisPool, RedisStorageError> {
        let pool_size = self.config.pool_size;

        let pool = self
            .config
            .into_fred_builder()
            .build_pool(pool_size)
            .map_err(RedisStorageError::from)?;

        // init() connects all pool members concurrently and waits until every
        // member has established its initial connection. Dropping the returned
        // ConnectHandle does NOT cancel the background I/O tasks.
        let _ = pool
            .init()
            .await
            .map_err(RedisStorageError::from)?;

        // Register event listeners on each individual pool member — fred's Pool
        // does not implement EventInterface directly.
        for client in pool.clients() {
            spawn_event_listener(client);
        }

        tracing::info!(pool_size, "redis.pool connected");

        Ok(RedisPool { inner: pool })
    }
}
