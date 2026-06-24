use std::ops::Deref;

use fred::clients::Client;
use fred::interfaces::ClientLike as _;

use crate::config::connection::RedisConfig;
use crate::error::map::RedisStorageError;
use crate::listener::event::spawn_event_listener;

/// A fully-initialised, multiplexed Redis client.
///
/// `RedisClient` is a thin newtype over `fred::clients::Client`. Fred
/// multiplexes an arbitrary number of concurrent commands over a single TCP
/// connection using a lock-free command queue, so one `RedisClient` is
/// sufficient for most services. Use [`crate::pool::builder::RedisPool`] when
/// a single connection becomes the bottleneck (bulk pipelines, very high
/// fan-out writes).
///
/// ## Cloning
///
/// `RedisClient` is cheaply cloneable — the inner fred client is backed by an
/// `Arc`. Pass clones to individual CQRS handlers or background tasks.
///
/// ## Usage
///
/// ```rust,ignore
/// use fred::interfaces::{ClientLike, KeysInterface};
/// use redis_storage::{RedisClientBuilder, RedisConfig};
///
/// let config = RedisConfig::from_env();
/// let client = RedisClientBuilder::new(config).build().await?;
///
/// client.set::<(), _, _>("session:42", "payload", None, None, false).await?;
/// let val: Option<String> = client.get("session:42").await?;
/// ```
#[derive(Clone)]
pub struct RedisClient {
    /// The underlying fred multiplexed client.
    ///
    /// Import `fred::interfaces::ClientLike` (and other command traits such as
    /// `KeysInterface`, `HashesInterface`, etc.) to issue Redis commands
    /// directly on this field, or use the `Deref` impl to call methods
    /// directly on `RedisClient`.
    pub inner: Client,
}

impl Deref for RedisClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Orchestrates `RedisClient` construction from a [`RedisConfig`].
///
/// Performs the following steps in order:
/// 1. Converts [`RedisConfig`] into a fred `Builder` (topology resolution +
///    performance/connection tuning).
/// 2. Calls `Builder::build()` to instantiate the fred `Client`.
/// 3. Calls `client.init().await` to establish the initial connection and
///    wait until it is ready (subject to `fail_fast` and connection timeout).
/// 4. Spawns the OTel event listener (see [`spawn_event_listener`]).
///
/// ## Errors
///
/// Returns [`RedisStorageError::Configuration`] when the provided config is
/// structurally invalid (e.g., empty host list). Returns
/// [`RedisStorageError::Disconnected`] when no Redis node is reachable within
/// the configured `connection_timeout` and `fail_fast` is `true`.
pub struct RedisClientBuilder {
    config: RedisConfig,
}

impl RedisClientBuilder {
    pub fn new(config: RedisConfig) -> Self {
        Self { config }
    }

    /// Connects to Redis and returns a [`RedisClient`] ready for use.
    ///
    /// # Errors
    ///
    /// See [`RedisClientBuilder`] error documentation.
    #[tracing::instrument(
        name   = "redis.client.build",
        skip(self),
        fields(topology = ?self.config.topology)
    )]
    pub async fn build(self) -> Result<RedisClient, RedisStorageError> {
        let builder = self.config.into_fred_builder();

        let client = builder
            .build()
            .map_err(RedisStorageError::from)?;

        // init() connects the client and waits until the initial connection is
        // established. The returned ConnectHandle drives the background I/O
        // loop; dropping it does NOT cancel the task.
        let _ = client
            .init()
            .await
            .map_err(RedisStorageError::from)?;

        spawn_event_listener(&client);

        tracing::info!("redis.client connected");

        Ok(RedisClient { inner: client })
    }
}
