use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, PhoneNumber};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneChanged {
    pub account_id: AccountId,
    pub new_phone: Option<PhoneNumber>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
