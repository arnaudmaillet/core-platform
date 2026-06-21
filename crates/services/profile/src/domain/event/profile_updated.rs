use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::ProfileId;

#[derive(Debug, Clone)]
pub struct ProfileUpdated {
    pub profile_id: ProfileId,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
