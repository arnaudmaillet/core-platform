use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_object::{ProfileId, VerificationKind};

#[derive(Debug, Clone)]
pub struct ProfileVerified {
    pub profile_id: ProfileId,
    pub verification_kind: VerificationKind,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
