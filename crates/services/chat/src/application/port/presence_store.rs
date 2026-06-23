use async_trait::async_trait;

use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;

/// Member-Plane presence and typing signals.
///
/// Strictly Member-Plane: only members heartbeat here and only members read it,
/// so this high-frequency churn never reaches the Audience Plane. Backed by
/// expiring sorted sets keyed under the conversation hash tag, so all presence
/// state for one conversation shares a single Redis Cluster slot and is read in
/// one round-trip (bounded to <= 500 members).
#[async_trait]
pub trait PresenceStore: Send + Sync + 'static {
    /// Records that `member_id` is online as of `now_ms`. Stale members (last seen
    /// before `now_ms - ttl_secs`) are pruned opportunistically.
    async fn heartbeat(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError>;

    /// Lists members seen within the last `ttl_secs`.
    async fn online(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<ProfileId>, ChatError>;

    /// Removes a member from presence immediately (clean disconnect).
    async fn leave(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<(), ChatError>;

    /// Marks `member_id` as typing, expiring after `ttl_secs` (short by design).
    async fn start_typing(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<(), ChatError>;

    /// Lists members currently typing (within the last `ttl_secs`).
    async fn typing(
        &self,
        conversation_id: &ConversationId,
        now_ms:          i64,
        ttl_secs:        u64,
    ) -> Result<Vec<ProfileId>, ChatError>;
}
