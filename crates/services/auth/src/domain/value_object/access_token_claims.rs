use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{AccountId, Generation, Permission, SessionId};

/// The normalized claim set for an edge access token, produced by
/// [`Session::mint_access_token`](crate::domain::aggregate::Session::mint_access_token).
///
/// This is the **domain** view of a token — not its wire form. The infrastructure
/// `TokenMinterPort` (Phase 4) serializes it into an ES256 JWT (PASETO later);
/// the `auth-context` library reconstructs the same shape on verification. By
/// construction `expires_at` is clamped to the session's horizon, so the token's
/// lifetime is always a subset of its session's lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// `sub` — the internal account id (never the IdP subject).
    pub account_id: AccountId,
    /// `sid` — the session this token belongs to.
    pub session_id: SessionId,
    /// `gen` — the revocation epoch checked at the edge for instant logout.
    pub generation: Generation,
    /// Normalized authorization grants.
    pub permissions: Vec<Permission>,
    pub issued_at: DateTime<Utc>,
    /// Always ≤ the session's sliding and absolute expiry.
    pub expires_at: DateTime<Utc>,
}

impl AccessTokenClaims {
    pub(crate) fn new(
        account_id: AccountId,
        session_id: SessionId,
        generation: Generation,
        permissions: Vec<Permission>,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self { account_id, session_id, generation, permissions, issued_at, expires_at }
    }

    /// Remaining lifetime in whole seconds at `now` (saturating at zero).
    pub fn expires_in_secs(&self, now: DateTime<Utc>) -> i64 {
        (self.expires_at - now).num_seconds().max(0)
    }
}
