use scylla::value::CqlTimestamp;
use scylla::DeserializeRow;
use uuid::Uuid;

/// Positional deserialization for `post.posts`.
///
/// SELECT must emit columns in exactly this order:
/// post_id, profile_id, kind, status, caption, attachments,
/// parent_id, root_id, created_at, updated_at, published_at, deleted_at
#[derive(DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct PostRow {
    pub post_id:      Uuid,
    pub profile_id:   Uuid,
    pub kind:         i8,
    pub status:       i8,
    pub caption:      String,
    pub attachments:  String,
    pub parent_id:    Option<Uuid>,
    pub root_id:      Option<Uuid>,
    pub created_at:   CqlTimestamp,
    pub updated_at:   CqlTimestamp,
    pub published_at: Option<CqlTimestamp>,
    pub deleted_at:   Option<CqlTimestamp>,
}
