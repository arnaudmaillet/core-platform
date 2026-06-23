use std::ops::Deref;

use fred::clients::SubscriberClient;
use fred::interfaces::ClientLike as _;

use crate::config::connection::RedisConfig;
use crate::error::map::RedisStorageError;
use crate::listener::event::spawn_event_listener;

/// A connected fred [`SubscriberClient`] dedicated to publish/subscribe.
///
/// A `SubscriberClient` keeps its own connection so subscription traffic never
/// shares the multiplexed [`RedisClient`](crate::client::builder::RedisClient)
/// command connection. It also tracks its channel set and re-subscribes
/// automatically after a reconnect (via `manage_subscriptions`, spawned by the
/// builder), which is essential for long-lived gRPC streaming workers in a
/// cluster that may fail over.
///
/// Publish (`SPUBLISH`) is issued on the regular `RedisClient`; this client is
/// for the subscribe side (`SSUBSCRIBE` / `message_rx`).
///
/// ## Cloning
///
/// Cheaply cloneable — the inner client is `Arc`-backed.
#[derive(Clone)]
pub struct RedisSubscriber {
    /// The underlying fred subscriber client. Import `fred::interfaces::{ClientLike,
    /// PubsubInterface, EventInterface}` to drive it directly, or use `Deref`.
    pub inner: SubscriberClient,
}

impl Deref for RedisSubscriber {
    type Target = SubscriberClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Builds a connected [`RedisSubscriber`] from a [`RedisConfig`].
///
/// Mirrors [`RedisClientBuilder`](crate::client::builder::RedisClientBuilder):
/// resolves topology, connects, spawns the OTel event listener, and additionally
/// spawns the automatic re-subscription manager.
pub struct RedisSubscriberBuilder {
    config: RedisConfig,
}

impl RedisSubscriberBuilder {
    pub fn new(config: RedisConfig) -> Self {
        Self { config }
    }

    #[tracing::instrument(name = "redis.subscriber.build", skip(self))]
    pub async fn build(self) -> Result<RedisSubscriber, RedisStorageError> {
        let builder = self.config.into_fred_builder();

        let subscriber = builder
            .build_subscriber_client()
            .map_err(RedisStorageError::from)?;

        // Bind (not `let _`) so the ConnectHandle is dropped at end of scope
        // rather than immediately; dropping it does not cancel the I/O task.
        let _connect = subscriber
            .init()
            .await
            .map_err(RedisStorageError::from)?;

        // Re-subscribe to all tracked channels after any reconnect.
        subscriber.manage_subscriptions();
        spawn_event_listener(&subscriber);

        tracing::info!("redis.subscriber connected");

        Ok(RedisSubscriber { inner: subscriber })
    }
}
