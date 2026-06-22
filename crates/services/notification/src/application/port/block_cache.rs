use async_trait::async_trait;

use crate::domain::value_object::ProfileId;
use crate::error::NotificationError;

/// Port for checking whether a notification should be suppressed due to a
/// social-graph block relationship.
///
/// The implementation caches results in Redis (`notification:block:{actor}:{target}`
/// with a short TTL) to avoid cross-service gRPC calls on every incoming event.
///
/// Cache miss policy: treat as "not blocked" (false-positive safe — the worst
/// outcome is a notification delivered to a user who recently set a block, which
/// the client UI will filter via the social-graph service). This is preferable to
/// the alternative of dropping legitimate notifications due to stale cache.
///
/// The social-graph service is expected to proactively invalidate or populate
/// block cache entries when blocks are created or removed. The notification
/// service itself never writes block state — it is read-only from this boundary.
#[async_trait]
pub trait BlockCache: Send + Sync + 'static {
    /// Returns `true` if `target_profile_id` has blocked `sender_profile_id`.
    /// A `true` result means the notification MUST be dropped silently.
    async fn is_blocked(
        &self,
        sender_profile_id: &ProfileId,
        target_profile_id: &ProfileId,
    ) -> Result<bool, NotificationError>;
}
