use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::domain::value_object::{NotificationKind, ProfileId, SubjectKind};

/// Lightweight payload broadcast to active gRPC streaming clients.
/// Uses plain scalar types to allow zero-copy broadcast across threads.
#[derive(Debug, Clone)]
pub struct NotificationPayload {
    pub notification_id:   Uuid,
    pub target_profile_id: Uuid,
    pub sender_profile_id: Uuid,
    pub sample_sender_ids: Vec<Uuid>,
    pub sender_count:      i32,
    pub kind:              NotificationKind,
    pub subject_kind:      SubjectKind,
    pub subject_id:        Uuid,
    pub created_at_ms:     i64,
}

/// Port for dispatching real-time notifications to active gRPC streaming clients.
///
/// Only profiles with an active `StreamNotifications` connection receive pushes.
/// The stream is best-effort: callers MUST NOT rely on this for durability.
/// Durable delivery is provided exclusively by the ScyllaDB read model.
pub trait StreamRegistry: Send + Sync + 'static {
    /// Registers a new subscriber for `profile_id` and returns a receiver.
    /// The BFF/gRPC handler calls this when a `StreamNotifications` stream opens.
    fn subscribe(
        &self,
        profile_id: &ProfileId,
    ) -> broadcast::Receiver<Arc<NotificationPayload>>;

    /// Broadcasts `payload` to all active subscribers for `profile_id`.
    /// Silently succeeds if there are no active subscribers (no-op).
    /// If a subscriber's buffer is full, it receives `RecvError::Lagged`.
    fn broadcast(&self, profile_id: &ProfileId, payload: Arc<NotificationPayload>);
}
