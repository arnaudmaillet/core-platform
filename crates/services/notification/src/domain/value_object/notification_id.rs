use std::fmt;
use uuid::Uuid;

use crate::error::NotificationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(Uuid);

/// Fixed namespace for deterministic (UUIDv5) notification identifiers.
///
/// Generated once and frozen forever: changing it would re-key every existing
/// notification and break idempotency across a deployment boundary.
const NOTIFICATION_NAMESPACE: Uuid = Uuid::from_u128(0x9d2f7e54_4b1c_4f6a_9c3d_2a7b8e0f1c63);

impl NotificationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Derives a stable, collision-free identifier from a business key (UUIDv5).
    ///
    /// The same `business_key` always yields the same id, which makes the
    /// `notifications_by_profile` INSERT naturally idempotent: a redelivered Kafka
    /// event overwrites the identical row instead of appending a duplicate. The
    /// `business_key` must encode the source event's stable identity (e.g. the
    /// comment id, or `post_id:mentioned_profile_id`).
    pub fn deterministic(business_key: &str) -> Self {
        Self(Uuid::new_v5(&NOTIFICATION_NAMESPACE, business_key.as_bytes()))
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<&str> for NotificationId {
    type Error = NotificationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| NotificationError::InvalidNotificationId(s.to_owned()))
    }
}

impl fmt::Display for NotificationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
