use scylla::value::CqlTimestamp;
use scylla::DeserializeRow;
use uuid::Uuid;

/// Positional deserialization target for rows from the `followers` and `following` tables.
///
/// SELECT must always emit `(profile_id_column, followed_at)` in this order.
/// `enforce_order` matches fields by position rather than by column name, so the
/// same struct serves both tables regardless of whether the first column is named
/// `follower_id` or `followee_id`.
#[derive(DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct FollowRow {
    pub profile_id:  Uuid,
    pub followed_at: CqlTimestamp,
}
