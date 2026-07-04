use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::value_object::{ActorId, EnforcementVersion};
use crate::error::ModerationError;

/// The hot-path **enforcement projection** (Plane B) — a Redis-backed, O(1)
/// "is this actor restricted right now" view that producing services consult
/// synchronously (`mod:enf:{actor:<id>}`), instead of calling moderation per item.
///
/// Writes carry the [`EnforcementVersion`] so the adapter can reject a stale
/// update (a reversal that arrives after a newer re-application must not clear the
/// newer restriction). The projection is denormalized; the durable enforcement
/// record in Postgres remains the source of truth.
#[async_trait]
pub trait EnforcementProjection: Send + Sync + 'static {
    /// Marks an actor restricted at `version`, optionally with an expiry (after
    /// which the restriction lapses). A write at a lower version than the stored
    /// one is ignored.
    async fn set_actor_restriction(
        &self,
        actor_id: &ActorId,
        version: EnforcementVersion,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), ModerationError>;

    /// Clears an actor's restriction, but only if `version` is at least the stored
    /// one (a stale reversal is ignored).
    async fn clear_actor_restriction(
        &self,
        actor_id: &ActorId,
        version: EnforcementVersion,
    ) -> Result<(), ModerationError>;

    /// Whether the actor is currently restricted.
    async fn is_actor_restricted(&self, actor_id: &ActorId) -> Result<bool, ModerationError>;
}
