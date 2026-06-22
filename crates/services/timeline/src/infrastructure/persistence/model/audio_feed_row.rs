use scylla::DeserializeRow;
use scylla::value::CqlTimestamp;
use uuid::Uuid;

/// ScyllaDB row type for `timeline.posts_by_audio`.
///
/// Column order MUST match the SELECT column list exactly.
#[derive(Debug, DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct AudioFeedRow {
    pub audio_id:     Uuid,
    pub published_at: CqlTimestamp,
    pub post_id:      Uuid,
    pub author_id:    Uuid,
}
