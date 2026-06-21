use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::AccountId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdprDeletionRequested {
    pub account_id: AccountId,
    pub retention_days: u32,
    pub scheduled_deletion_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
