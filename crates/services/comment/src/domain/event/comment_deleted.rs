use serde::{Deserialize, Serialize};

/// Emitted when a comment is removed (soft-delete tombstone or physical purge).
///
/// Published to Kafka topic `comment.deleted` (key: `comment_id`).
/// The engagement service's CommentEventConsumer reads `post_id` from this
/// payload to decrement its Redis and ScyllaDB counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentDeletedEvent {
    pub comment_id:    String,
    pub post_id:       String,
    pub author_id:     String,
    pub deleted_at_ms: i64,
}
