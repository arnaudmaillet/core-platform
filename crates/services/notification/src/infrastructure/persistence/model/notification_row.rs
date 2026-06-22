use scylla::DeserializeRow;
use scylla::value::CqlTimestamp;
use uuid::Uuid;

/// ScyllaDB row type for `notification.notifications_by_profile`.
///
/// Column order MUST match the SELECT column list exactly due to
/// `scylla(flavor = "enforce_order")` — ScyllaDB 1.x deserializes by position.
#[derive(Debug, DeserializeRow)]
#[scylla(flavor = "enforce_order")]
pub struct NotificationRow {
    pub target_profile_id: Uuid,
    pub created_at:        CqlTimestamp,
    pub notification_id:   Uuid,
    pub notification_kind: i8,
    pub subject_kind:      i8,
    pub subject_id:        Uuid,
    pub sender_profile_id: Uuid,
    pub sender_count:      i32,
    pub sample_sender_ids: Vec<Uuid>,
    pub is_read:           bool,
}
