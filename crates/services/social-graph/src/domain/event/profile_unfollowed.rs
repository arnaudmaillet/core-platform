use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUnfollowed {
    pub actor_id:      ProfileId,
    pub target_id:     ProfileId,
    pub unfollowed_at: DateTime<Utc>,
}
