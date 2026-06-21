use scylla::value::CqlTimestamp;
use scylla::DeserializeRow;
use uuid::Uuid;

/// Positional deserialization target for rows from the `blocks` table.
///
/// SELECT must emit `(blockee_id, blocked_at)` in this order.
#[derive(DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct BlockRow {
    pub blockee_id: Uuid,
    pub blocked_at: CqlTimestamp,
}
