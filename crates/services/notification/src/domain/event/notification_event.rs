use serde::{Deserialize, Serialize};

/// Domain events emitted within the notification service boundary.
/// Currently informational — not published to an external Kafka topic.
/// Future: publish to `notification.delivered` for downstream analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum NotificationEvent {
    Created(NotificationCreatedEvent),
    Read(NotificationReadEvent),
}

/// Emitted after a notification row is durably written to ScyllaDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationCreatedEvent {
    pub notification_id:   String,
    pub target_profile_id: String,
    pub sender_profile_id: String,
    pub sender_count:      i32,
    pub kind:              String,
    pub subject_kind:      String,
    pub subject_id:        String,
    pub created_at_ms:     i64,
}

/// Emitted when a notification transitions from unread to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationReadEvent {
    pub notification_id:   String,
    pub target_profile_id: String,
    pub read_at_ms:        i64,
}
