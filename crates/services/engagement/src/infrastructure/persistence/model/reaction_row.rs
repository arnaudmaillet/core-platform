use scylla::DeserializeRow;
use scylla::value::CqlTimestamp;
use uuid::Uuid;

/// ScyllaDB row type for `engagement.post_reactions`.
#[derive(Debug, DeserializeRow)]
pub struct ReactionRow {
    pub post_id:    Uuid,
    pub profile_id: Uuid,
    pub kind:       i8,
    pub weight:     i32,
    pub reacted_at: CqlTimestamp,
}
