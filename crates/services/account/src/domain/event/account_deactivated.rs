use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::AccountId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDeactivated {
    pub account_id: AccountId,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
