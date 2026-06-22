use scylla::DeserializeRow;
use scylla::value::CqlTimestamp;
use uuid::Uuid;

/// ScyllaDB row type for `timeline.feed_items_by_profile`.
///
/// Column order MUST match the SELECT column list exactly — ScyllaDB 1.x
/// deserializes by position with `scylla(flavor = "enforce_order")`.
#[derive(Debug, DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct FeedItemRow {
    pub profile_id:   Uuid,
    pub published_at: CqlTimestamp,
    pub post_id:      Uuid,
    pub author_id:    Uuid,
}
