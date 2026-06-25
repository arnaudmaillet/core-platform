use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::value_object::{AccountId, Generation, RevocationReason, SessionId};

/// A session was revoked. For a global sign-out, one event is emitted per
/// affected session and `reason` is `GlobalLogout`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevoked {
    pub session_id: SessionId,
    pub account_id: AccountId,
    /// The generation the session was minted under.
    pub generation: Generation,
    pub reason: RevocationReason,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: Uuid,
}
