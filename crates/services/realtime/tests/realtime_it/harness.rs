//! Integration harness: boots an ephemeral Redis, wires the real realtime routing
//! adapters against it, and provides builders for scenario-isolated identities.
#![allow(dead_code)]

use std::sync::Arc;

use chrono::Utc;
use redis_storage::{RedisClient, RedisClientBuilder, RedisConfig, RedisSubscriber, RedisSubscriberBuilder};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use realtime::application::port::{ConnectionLocation, NodeChannel};
use realtime::application::{DeliverableEvent, FanOutHandler};
use realtime::domain::{
    ChannelClass, ChannelKey, ChannelRef, Connection, ConnectionId, DeviceId, NodeId, Session,
    UserId,
};
use realtime::infrastructure::redis_connection_registry::RedisConnectionRegistry;
use realtime::infrastructure::redis_node_channel::RedisNodeChannel;
use realtime::infrastructure::runtime::{ConnHandle, ConnectionTable};

/// Short registry TTL so the self-heal scenario can observe expiry quickly.
pub const SHORT_TTL_MS: i64 = 1_000;
pub const LONG_TTL_MS: i64 = 120_000;

pub struct Harness {
    pub redis: RedisClient,
    pub registry: Arc<RedisConnectionRegistry>,
    pub node_channel: Arc<RedisNodeChannel>,
    redis_config: RedisConfig,
}

impl Harness {
    pub async fn start() -> Self {
        Self::with_ttl(LONG_TTL_MS).await
    }

    pub async fn with_ttl(ttl_ms: i64) -> Self {
        let endpoint = test_support::containers::redis_endpoint().await;
        let redis_config = RedisConfig {
            hosts: vec![endpoint],
            ..RedisConfig::default()
        };
        let redis = RedisClientBuilder::new(redis_config.clone())
            .build()
            .await
            .expect("it: redis client");

        let registry = Arc::new(RedisConnectionRegistry::new(redis.clone(), ttl_ms));
        let node_channel = Arc::new(RedisNodeChannel::new(redis.clone()));

        Self {
            redis,
            registry,
            node_channel,
            redis_config,
        }
    }

    /// A dedicated subscriber connection (for the gateway-side `SSUBSCRIBE` loop).
    pub async fn subscriber(&self) -> RedisSubscriber {
        RedisSubscriberBuilder::new(self.redis_config.clone())
            .build()
            .await
            .expect("it: redis subscriber")
    }

    pub fn fan_out(&self) -> FanOutHandler {
        FanOutHandler::new(self.registry.clone(), self.node_channel.clone())
    }
}

// ── Builders ──────────────────────────────────────────────────────────────────

/// A fresh, scenario-isolated user id.
pub fn fresh_user() -> UserId {
    UserId::new(format!("u-{}", Uuid::now_v7())).unwrap()
}

pub fn dm_channel(user: &UserId) -> ChannelRef {
    ChannelRef::new(
        ChannelClass::Dm,
        ChannelKey::new(user.as_str().to_owned()).unwrap(),
    )
}

pub fn location(user: &UserId, device: &str, conn: &str, node: &str) -> ConnectionLocation {
    ConnectionLocation {
        user_id: user.clone(),
        device_id: DeviceId::new(device).unwrap(),
        connection_id: ConnectionId::new(conn).unwrap(),
        node_id: NodeId::new(node).unwrap(),
    }
}

pub fn dm_event(user: &UserId, payload: &[u8]) -> DeliverableEvent {
    DeliverableEvent {
        recipient: Some(user.clone()),
        device_id: None,
        channel: dm_channel(user),
        payload: payload.to_vec(),
        event_type: "chat.message".to_owned(),
        emitted_at: Utc::now(),
        idempotency_key: Uuid::now_v7().to_string(),
    }
}

pub fn counter_channel(entity: &str) -> ChannelRef {
    ChannelRef::new(ChannelClass::Counter, ChannelKey::new(entity.to_owned()).unwrap())
}

/// A public broadcast event (no recipient) on `counter:<entity>`.
pub fn counter_event(entity: &str, payload: &[u8]) -> DeliverableEvent {
    DeliverableEvent {
        recipient: None,
        device_id: None,
        channel: counter_channel(entity),
        payload: payload.to_vec(),
        event_type: "counter.popularity".to_owned(),
        emitted_at: Utc::now(),
        idempotency_key: Uuid::now_v7().to_string(),
    }
}

/// Register a connection subscribed to `counter:<entity>` in both the user index
/// and the broadcast (channel) index, and return its socket receiver.
pub async fn register_broadcast_subscriber(
    table: &ConnectionTable,
    user: &UserId,
    entity: &str,
    conn: &str,
    node: &str,
    queue_cap: usize,
) -> mpsc::Receiver<Vec<u8>> {
    let session = Session::new(
        user.clone(),
        DeviceId::new("phone").unwrap(),
        Utc::now() + chrono::Duration::hours(1),
    );
    let mut connection = Connection::open(
        ConnectionId::new(conn).unwrap(),
        NodeId::new(node).unwrap(),
        session,
        16,
        Utc::now(),
    );
    let channel = counter_channel(entity);
    connection.subscribe(channel.clone()).unwrap();

    let (tx, rx) = mpsc::channel(queue_cap);
    let handle = ConnHandle {
        connection_id: ConnectionId::new(conn).unwrap(),
        device_id: DeviceId::new("phone").unwrap(),
        connection: Arc::new(Mutex::new(connection)),
        sender: tx,
    };
    table.subscribe_channel(&channel.to_string(), handle.clone());
    table.insert(user.as_str(), handle);
    rx
}

/// Register a connection in `table` subscribed to its `dm:<user>` channel, and
/// return the receiver standing in for its socket writer.
pub async fn register_connection(
    table: &ConnectionTable,
    user: &UserId,
    device: &str,
    conn: &str,
    node: &str,
    queue_cap: usize,
) -> mpsc::Receiver<Vec<u8>> {
    let session = Session::new(
        user.clone(),
        DeviceId::new(device).unwrap(),
        Utc::now() + chrono::Duration::hours(1),
    );
    let mut connection = Connection::open(
        ConnectionId::new(conn).unwrap(),
        NodeId::new(node).unwrap(),
        session,
        16,
        Utc::now(),
    );
    connection.subscribe(dm_channel(user)).unwrap();

    let (tx, rx) = mpsc::channel(queue_cap);
    table.insert(
        user.as_str(),
        ConnHandle {
            connection_id: ConnectionId::new(conn).unwrap(),
            device_id: DeviceId::new(device).unwrap(),
            connection: Arc::new(Mutex::new(connection)),
            sender: tx,
        },
    );
    rx
}

/// Publish to a node's channel directly (test convenience).
pub async fn publish(channel: &RedisNodeChannel, node: &str, event: &DeliverableEvent) {
    channel
        .publish(&NodeId::new(node).unwrap(), event)
        .await
        .expect("it: node publish");
}
