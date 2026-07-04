use async_trait::async_trait;
use fred::interfaces::PubsubInterface;
use redis_storage::RedisClient;

use crate::domain::value_object::ConversationId;
use crate::error::ChatError;
use crate::infrastructure::cache::redis_err;
use crate::infrastructure::routing::channel::ChannelScheme;
use crate::infrastructure::routing::plane::PlaneEvent;

/// Publishes plane events to the sharded pub/sub backbone via `SPUBLISH`.
///
/// `SPUBLISH` (sharded pub/sub) confines delivery to the shard owning the
/// channel's slot, so member traffic stays on the conversation's home slot and
/// audience traffic stays on its spread shard — neither floods the whole cluster
/// the way classic `PUBLISH` would.
#[async_trait]
pub trait PlaneBroadcaster: Send + Sync + 'static {
    /// Publishes to the Member-Plane channel (full event: messages + presence +
    /// typing + receipts).
    async fn broadcast_member(
        &self,
        conversation_id: &ConversationId,
        event:           &PlaneEvent,
    ) -> Result<(), ChatError>;

    /// Publishes to one Audience-Plane shard channel (message shadow only).
    async fn broadcast_audience(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
        event:           &PlaneEvent,
    ) -> Result<(), ChatError>;
}

pub struct RedisPlaneBroadcaster {
    client: RedisClient,
}

impl RedisPlaneBroadcaster {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlaneBroadcaster for RedisPlaneBroadcaster {
    async fn broadcast_member(
        &self,
        conversation_id: &ConversationId,
        event:           &PlaneEvent,
    ) -> Result<(), ChatError> {
        let payload = event.to_json()?;
        let _: i64 = self
            .client
            .inner
            .spublish(ChannelScheme::member_channel(conversation_id), payload)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn broadcast_audience(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
        event:           &PlaneEvent,
    ) -> Result<(), ChatError> {
        let payload = event.to_json()?;
        let _: i64 = self
            .client
            .inner
            .spublish(ChannelScheme::audience_channel(conversation_id, shard), payload)
            .await
            .map_err(redis_err)?;
        Ok(())
    }
}
