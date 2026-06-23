use async_trait::async_trait;

use crate::domain::aggregate::Conversation;
use crate::domain::value_object::ConversationId;
use crate::error::ChatError;

/// Persistence port for the [`Conversation`] aggregate (`chat.conversations`).
///
/// Backed by a single point-read/point-write per call — the aggregate is small
/// and bounded by design, and the audience is never loaded through it.
#[async_trait]
pub trait ConversationRepository: Send + Sync + 'static {
    /// Inserts a freshly created conversation.
    async fn insert(&self, conversation: &Conversation) -> Result<(), ChatError>;

    /// Persists the mutable aggregate state after a transition: `visibility`,
    /// `public_since`, `member_count`, and `updated_at`. The immutable columns
    /// (`kind`, `owner_id`, `created_at`) are not rewritten.
    async fn update(&self, conversation: &Conversation) -> Result<(), ChatError>;

    /// Reconstitutes the aggregate by id, or `None` if it does not exist.
    async fn find(&self, id: &ConversationId) -> Result<Option<Conversation>, ChatError>;
}
