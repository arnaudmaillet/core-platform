use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::AccountId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdprDataExportRequested {
    pub account_id: AccountId,
    pub requested_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
