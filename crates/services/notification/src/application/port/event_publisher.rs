use async_trait::async_trait;
use serde_json::Value;

use crate::error::NotificationError;

/// A created-notification signal published to `notification.v1.events` and
/// consumed by `realtime` for live device push.
///
/// The wire body mirrors realtime's `NotificationWire` (recipient_id /
/// notification_id / kind / created_at_ms / payload). `payload` is the
/// already-rendered, client-ready view the realtime plane forwards verbatim —
/// it is never interpreted downstream.
#[derive(Debug, Clone)]
pub struct NotificationStreamEvent {
    /// The recipient (target profile id); realtime addresses this identity's
    /// `notif` channel and keys the partition.
    pub recipient_id:    String,
    /// Stable notification id — realtime's delivery idempotency key.
    pub notification_id: String,
    /// Notification kind (`follow`, `comment`, …); realtime tags the event
    /// `notif.{kind}`.
    pub kind:            String,
    pub created_at_ms:   i64,
    pub payload:         Value,
}

/// Publishes created notifications to the realtime push stream.
///
/// Best-effort at the call site: the notification is already durably written, so
/// a push failure must never roll back the command (mirrors the in-process
/// stream-registry broadcast, which is also best-effort).
#[async_trait]
pub trait NotificationEventPublisher: Send + Sync {
    async fn publish(&self, event: &NotificationStreamEvent) -> Result<(), NotificationError>;
}
