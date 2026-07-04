use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, EmailAddress};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerified {
    pub account_id: AccountId,
    pub email: EmailAddress,
    pub verified_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
