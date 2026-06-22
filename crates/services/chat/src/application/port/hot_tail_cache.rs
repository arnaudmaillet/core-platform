use async_trait::async_trait;

use crate::application::port::MessageSummary;
use crate::domain::value_object::ConversationId;
use crate::error::ChatError;

/// Per-conversation hot-tail cache — the read offload that keeps passive readers
/// off the live ScyllaDB write partition.
///
/// Holds the most recent `cap` messages of a conversation as a capped, newest-
/// first structure. "Load last page" and short scroll-ups are served from here;
/// ScyllaDB is only touched on a cache miss into cold history. Best-effort:
/// ScyllaDB remains the durable source of truth, so a cold/empty cache is always
/// safe to rebuild from it.
#[async_trait]
pub trait HotTailCache: Send + Sync + 'static {
    /// Appends a message to the tail and trims it back to `cap`.
    async fn push(
        &self,
        conversation_id: &ConversationId,
        message:         &MessageSummary,
        cap:             u16,
    ) -> Result<(), ChatError>;

    /// Returns the newest `limit` messages, newest-first.
    async fn recent(
        &self,
        conversation_id: &ConversationId,
        limit:           usize,
    ) -> Result<Vec<MessageSummary>, ChatError>;

    /// Returns up to `limit` messages with `created_at_ms <= max_score_inclusive`,
    /// newest-first — the in-cache scroll-up page. An empty result means the
    /// requested window has aged out of the cache and the caller must fall back to
    /// ScyllaDB.
    async fn range_desc(
        &self,
        conversation_id:     &ConversationId,
        max_score_inclusive: i64,
        limit:               usize,
    ) -> Result<Vec<MessageSummary>, ChatError>;

    /// Whether a tail cache currently exists for the conversation (warm check).
    async fn exists(&self, conversation_id: &ConversationId) -> Result<bool, ChatError>;
}
