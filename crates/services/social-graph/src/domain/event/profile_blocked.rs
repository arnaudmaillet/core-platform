use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileBlocked {
    pub actor_id:   ProfileId,
    pub target_id:  ProfileId,
    pub blocked_at: DateTime<Utc>,
    /// True if the block severed an existing actor→target follow.
    pub severed_actor_follow:  bool,
    /// True if the block severed an existing target→actor follow.
    pub severed_target_follow: bool,
}
