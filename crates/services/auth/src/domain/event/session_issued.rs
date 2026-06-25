use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, Generation, IdpSubject, SessionId};

/// A new session was established for an account (successful login).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIssued {
    pub session_id: SessionId,
    pub account_id: AccountId,
    pub subject: IdpSubject,
    pub generation: Generation,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub absolute_expiry: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
