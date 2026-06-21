use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::{MaskingReason, ProfileId};

#[derive(Debug, Clone)]
pub struct ProfileHidden {
    pub profile_id: ProfileId,
    pub masking_reason: MaskingReason,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
