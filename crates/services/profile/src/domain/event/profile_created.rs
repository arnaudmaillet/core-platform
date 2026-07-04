use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, Handle, ProfileId, ProfileKind};

#[derive(Debug, Clone)]
pub struct ProfileCreated {
    pub profile_id: ProfileId,
    pub account_id: AccountId,
    pub handle: Handle,
    pub profile_kind: ProfileKind,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
