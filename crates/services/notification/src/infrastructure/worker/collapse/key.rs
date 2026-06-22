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

    /// Redis key for the cross-batch collapse window counter.
    ///
    /// The `(target, subject, kind)` identity is wrapped in a Redis Cluster hash
    /// tag `{...}` so this key and [`Self::redis_senders_key`] always hash to the
    /// same slot. The collapse/drain Lua scripts touch both keys atomically, so
    /// they must co-locate — otherwise the server rejects them with CROSSSLOT.
    /// Format: `notification:cw:{<target>:<subject>:<kind>}`
    pub fn redis_window_key(&self) -> String {
        format!(
            "notification:cw:{{{}:{}:{}}}",
            self.target_profile_id,
            self.subject_id,
            self.kind.as_str(),
        )
    }

    /// Redis key for the RPUSH sender sample list (parallel to `redis_window_key`).
    /// Inherits the window's hash tag, so it shares the same cluster slot.
    pub fn redis_senders_key(&self) -> String {
        format!("{}_:senders", self.redis_window_key())
    }

    /// Redis key for the SET of unique senders in this window. Used to make
    /// accumulation idempotent — a sender that has already reacted within the
    /// window does not increment the count again. Inherits the window's hash tag.
    pub fn redis_senders_set_key(&self) -> String {
        format!("{}_:sset", self.redis_window_key())
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
