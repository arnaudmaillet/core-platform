use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, KycStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycStatusChanged {
    pub account_id: AccountId,
    pub old_status: KycStatus,
    pub new_status: KycStatus,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
