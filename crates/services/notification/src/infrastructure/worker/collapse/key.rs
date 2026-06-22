use uuid::Uuid;

use crate::domain::value_object::{NotificationKind, SubjectKind};

/// Identifies a logical collapse group within a Kafka batch.
///
/// Events sharing the same `(target_profile_id, subject_id, kind)` tuple are
/// coalesced into a single ScyllaDB write by the in-batch accumulator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CollapseKey {
    pub target_profile_id: Uuid,
    pub subject_id:        Uuid,
    pub subject_kind:      SubjectKind,
    pub kind:              NotificationKind,
}

impl CollapseKey {
    pub fn new(
        target_profile_id: Uuid,
        subject_id:        Uuid,
        subject_kind:      SubjectKind,
        kind:              NotificationKind,
    ) -> Self {
        Self { target_profile_id, subject_id, subject_kind, kind }
    }

    /// Redis key used by the cross-batch collapse window and the schedule ZSET.
    /// Format: `notification:cw:{target}:{subject}:{kind}`
    pub fn redis_window_key(&self) -> String {
        format!(
            "notification:cw:{}:{}:{}",
            self.target_profile_id,
            self.subject_id,
            self.kind.as_str(),
        )
    }

    /// Redis key for the LPUSH sender sample list (parallel to `redis_window_key`).
    pub fn redis_senders_key(&self) -> String {
        format!("{}_:senders", self.redis_window_key())
    }

    /// Member string used in the `notification:window_schedule` ZSET.
    /// Format: `{target}:{subject}:{kind}`
    pub fn schedule_member(&self) -> String {
        format!(
            "{}:{}:{}",
            self.target_profile_id,
            self.subject_id,
            self.kind.as_str(),
        )
    }
}
