use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;

/// Persistence port for the Audience Plane subscription set.
///
/// Writes maintain two denormalized tables in lockstep:
/// `subscriptions_by_conversation` (hash-bucketed, for admin/analytics listing)
/// and `subscriptions_by_user` (the reverse index powering the O(1) membership
/// probe and "my subscriptions"). The pair is best-effort consistent — Scylla
/// offers no multi-partition transaction — matching the platform's other
/// denormalized indexes.
#[async_trait]
pub trait SubscriptionRepository: Send + Sync + 'static {
    /// Subscribes a profile to a public conversation (idempotent upsert).
    async fn subscribe(
        &self,
        conversation_id: &ConversationId,
        subscriber_id:   &ProfileId,
    ) -> Result<(), ChatError>;

    /// Removes a subscription (idempotent).
    async fn unsubscribe(
        &self,
        conversation_id: &ConversationId,
        subscriber_id:   &ProfileId,
    ) -> Result<(), ChatError>;

    /// O(1) membership probe via the reverse index — the hot check on the
    /// audience read path.
    async fn is_subscribed(
        &self,
        subscriber_id:   &ProfileId,
        conversation_id: &ConversationId,
    ) -> Result<bool, ChatError>;

    /// Lists the conversations a profile subscribes to, paginated by
    /// `conversation_id` clustering. `cursor` is the last `conversation_id` of
    /// the previous page; returns `(ids, next_cursor)`.
    async fn list_by_user(
        &self,
        subscriber_id: &ProfileId,
        limit:         i32,
        cursor:        Option<Uuid>,
    ) -> Result<(Vec<ConversationId>, Option<Uuid>), ChatError>;
}
