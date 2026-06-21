use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, AccountRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRevoked {
    pub account_id: AccountId,
    pub role: AccountRole,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
