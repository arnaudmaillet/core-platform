use async_trait::async_trait;

use crate::domain::value_object::{ConversationId, MessageId, ProfileId};
use crate::error::ChatError;

/// Live mirror of per-member read-receipt horizons for the Member Plane.
///
/// The durable horizon lives in `members_by_conversation.last_read` (ScyllaDB);
/// this cache holds the real-time copy the UI renders and the routing layer
/// broadcasts. A Redis hash under the conversation hash tag — bounded to
/// O(members) and read in a single `HGETALL`. Audience members generate no
/// receipts, so this never grows with the audience.
#[async_trait]
pub trait ReceiptStore: Send + Sync + 'static {
    /// Sets `member_id`'s read horizon to `last_read`.
    async fn set(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        last_read:       MessageId,
    ) -> Result<(), ChatError>;

    /// Returns every member's current read horizon.
    async fn all(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<(ProfileId, MessageId)>, ChatError>;
}
