use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::ProfileId;

/// The author tier changed (denormalized from `social-graph.author_tier_changed`).
/// Re-emitted on `profile.v1.events` so `post` can stamp the current tier onto new
/// posts. `tier` is the shared `u8` taxonomy (0=Standard, 1=Premium, 2=Vip).
#[derive(Debug, Clone)]
pub struct TierChanged {
    pub profile_id: ProfileId,
    pub tier: u8,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
