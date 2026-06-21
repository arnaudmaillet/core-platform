use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::{Handle, ProfileId};

#[derive(Debug, Clone)]
pub struct ProfileDeleted {
    pub profile_id: ProfileId,
    pub handle: Handle,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
