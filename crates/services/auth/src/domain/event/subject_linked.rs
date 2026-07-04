use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, IdpSubject};

/// An IdP subject was linked to an internal account for the first time. Emitted
/// once, on first login; the link is immutable thereafter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectLinked {
    pub account_id: AccountId,
    pub subject: IdpSubject,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
