use scylla::value::CqlTimestamp;
use scylla::DeserializeRow;
use uuid::Uuid;

/// Positional deserialization for `post.posts_by_profile`.
///
/// SELECT must emit columns in exactly this order:
/// created_at, post_id, kind, status
#[derive(DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct PostProfileRow {
    pub created_at: CqlTimestamp,
    pub post_id:    Uuid,
    pub kind:       i8,
    pub status:     i8,
}
