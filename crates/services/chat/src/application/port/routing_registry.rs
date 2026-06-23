use async_trait::async_trait;

use crate::domain::value_object::ConversationId;
use crate::error::ChatError;

/// Tracks which Audience-Plane shards a public conversation is currently fanned
/// out across.
///
/// A connected pod activates the shard it serves (and heartbeats it); the
/// publisher reads the active set to know which `chat:{aud:<id>:<k>}` sharded
/// channels to `SPUBLISH` the shadow message to — so a viral conversation's
/// fan-out spreads across the cluster instead of pinning to one slot, and a
/// shard with no live pods is never published to. Entries expire so a crashed pod
/// stops receiving fan-out without an explicit deactivate.
///
/// This bookkeeping is small per-conversation metadata and lives under the
/// conversation hash tag; the actual sharded broadcast channels (Phase 5)
/// deliberately use spreading tags.
#[async_trait]
pub trait RoutingRegistry: Send + Sync + 'static {
    /// Marks `shard` active for the conversation as of `now_ms`, with a liveness
    /// window of `ttl_secs` (refreshed on each pod heartbeat).
    async fn activate_shard(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError>;

    /// Removes `shard` from the active set (last local subscriber gone).
    async fn deactivate_shard(
        &self,
        conversation_id: &ConversationId,
        shard:           u16,
    ) -> Result<(), ChatError>;

    /// Returns the shards with a live pod (heartbeated within `ttl_secs`) — the
    /// set the publisher must fan out to.
    async fn active_shards(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<u16>, ChatError>;

    /// Clears every active shard for a conversation. Invoked when a conversation
    /// is unpublished so the publisher immediately stops fanning to a now-detached
    /// Audience Plane.
    async fn clear(&self, conversation_id: &ConversationId) -> Result<(), ChatError>;
}
