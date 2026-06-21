use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileFollowed {
    pub actor_id:    ProfileId,
    pub target_id:   ProfileId,
    pub followed_at: DateTime<Utc>,
}
