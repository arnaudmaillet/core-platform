//! The node-hop fabric over Redis sharded Pub/Sub (`SPUBLISH`), the same backbone
//! `chat` uses.
//!
//! The dispatcher publishes a resolved event — encoded as a `realtime.v1`
//! `DeliverEnvelope` — to the owning node's channel `rt:node:{<node_id>}`. The
//! node-id hash tag keeps each node's channel on a single cluster slot, so
//! `SPUBLISH` confines delivery to that shard instead of flooding the cluster the
//! way classic `PUBLISH` would. The gateway side (`SSUBSCRIBE` on its own node
//! channel) is wired in Phase 5.
//!
//! Publish is fire-and-forget and intentionally not a delivery confirmation: the
//! owning node may have just lost the connection (a stale registry), in which case
//! the event is dropped there — a fail-open miss the client recovers from on
//! reconnect. A fred fault is `RTM-4002 NodeChannelUnavailable` (retryable).

use async_trait::async_trait;
use fred::interfaces::PubsubInterface;
use prost::Message;
use redis_storage::RedisClient;

use crate::application::event::DeliverableEvent;
use crate::application::port::NodeChannel;
use crate::domain::NodeId;
use crate::error::RealtimeError;
use crate::infrastructure::codec;

fn node_channel(node_id: &NodeId) -> String {
    format!("rt:node:{{{}}}", node_id.as_str())
}

/// The single fleet-wide broadcast channel every gateway subscribes to. Hash-tag
/// keeps it on one cluster slot; broadcast rate is low (coarse counter/feed
/// signals), so the single-shard concentration is acceptable.
pub const BROADCAST_CHANNEL: &str = "rt:broadcast:{0}";

fn channel_err(e: fred::error::Error) -> RealtimeError {
    tracing::warn!(error = %e, "node channel redis error");
    RealtimeError::NodeChannelUnavailable
}

pub struct RedisNodeChannel {
    client: RedisClient,
}

impl RedisNodeChannel {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl NodeChannel for RedisNodeChannel {
    async fn publish(
        &self,
        node_id: &NodeId,
        event: &DeliverableEvent,
    ) -> Result<(), RealtimeError> {
        // prost encoding into a growable Vec is infallible.
        let payload = codec::envelope_to_pb(event).encode_to_vec();

        let _: i64 = self
            .client
            .inner
            .spublish(node_channel(node_id), payload)
            .await
            .map_err(channel_err)?;
        Ok(())
    }

    async fn broadcast(&self, event: &DeliverableEvent) -> Result<(), RealtimeError> {
        let payload = codec::envelope_to_pb(event).encode_to_vec();
        let _: i64 = self
            .client
            .inner
            .spublish(BROADCAST_CHANNEL, payload)
            .await
            .map_err(channel_err)?;
        Ok(())
    }
}
