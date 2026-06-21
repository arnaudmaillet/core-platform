use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, EmailAddress};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailChanged {
    pub account_id: AccountId,
    pub old_email: EmailAddress,
    pub new_email: EmailAddress,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
