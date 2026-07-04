use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{
    AccountId, RefreshTokenHash, RefreshTokenId, RefreshTokenStatus, SessionId,
};
use crate::error::AuthError;

/// Parameters to mint a fresh refresh token.
#[derive(Debug, Clone)]
pub struct RefreshTokenIssueParams {
    pub session_id: SessionId,
    pub account_id: AccountId,
    pub token_hash: RefreshTokenHash,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// A single refresh token in a session's rotation lineage.
///
/// Modelled as its own aggregate (its own row, keyed by [`RefreshTokenId`]) so a
/// rotation never has to load the whole chain. The plaintext is opaque to the
/// client; only its [`RefreshTokenHash`] is held.
///
/// # Invariant 2 — single-use rotation with reuse-detection
/// A token rotates exactly once (`Active → Rotated`). Presenting a token that is
/// already `Rotated` is a theft signal: [`RefreshToken::rotate`] returns
/// [`AuthError::RefreshTokenReuseDetected`], on which the application revokes the
/// session's entire generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    id: RefreshTokenId,
    session_id: SessionId,
    account_id: AccountId,
    token_hash: RefreshTokenHash,
    status: RefreshTokenStatus,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    /// Successor minted when this token was rotated.
    replaced_by: Option<RefreshTokenId>,
    version: i64,
}

impl RefreshToken {
    // ─── Construction ────────────────────────────────────────────────────────

    /// Mints a new `Active` refresh token. Rejects a non-positive lifetime.
    pub fn issue(params: RefreshTokenIssueParams) -> Result<Self, AuthError> {
        if params.expires_at <= params.issued_at {
            return Err(AuthError::DomainViolation {
                field: "refresh_token.expires_at".into(),
                message: "refresh-token expiry must be after its issue time".into(),
            });
        }
        Ok(Self {
            id: RefreshTokenId::new(),
            session_id: params.session_id,
            account_id: params.account_id,
            token_hash: params.token_hash,
            status: RefreshTokenStatus::Active,
            issued_at: params.issued_at,
            expires_at: params.expires_at,
            used_at: None,
            replaced_by: None,
            version: 0,
        })
    }

    /// Reconstructs from storage (no validation).
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: RefreshTokenId,
        session_id: SessionId,
        account_id: AccountId,
        token_hash: RefreshTokenHash,
        status: RefreshTokenStatus,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        used_at: Option<DateTime<Utc>>,
        replaced_by: Option<RefreshTokenId>,
        version: i64,
    ) -> Self {
        Self {
            id,
            session_id,
            account_id,
            token_hash,
            status,
            issued_at,
            expires_at,
            used_at,
            replaced_by,
            version,
        }
    }

    // ─── Queries ─────────────────────────────────────────────────────────────

    pub fn id(&self) -> RefreshTokenId {
        self.id
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn token_hash(&self) -> &RefreshTokenHash {
        &self.token_hash
    }

    pub fn status(&self) -> RefreshTokenStatus {
        self.status
    }

    pub fn replaced_by(&self) -> Option<RefreshTokenId> {
        self.replaced_by
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn used_at(&self) -> Option<DateTime<Utc>> {
        self.used_at
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    // ─── Commands ────────────────────────────────────────────────────────────

    /// Rotates this token (single-use), returning its `Active` successor.
    ///
    /// - `Active` + unexpired ⇒ marks self `Rotated`, links `replaced_by`, mints
    ///   the successor with `new_hash`.
    /// - already `Rotated` ⇒ [`AuthError::RefreshTokenReuseDetected`] (invariant 2).
    /// - `Revoked` ⇒ [`AuthError::RefreshTokenAlreadyRotated`] (session signed out).
    /// - expired ⇒ [`AuthError::RefreshTokenExpired`].
    pub fn rotate(
        &mut self,
        now: DateTime<Utc>,
        new_hash: RefreshTokenHash,
        new_expires_at: DateTime<Utc>,
    ) -> Result<RefreshToken, AuthError> {
        match self.status {
            RefreshTokenStatus::Rotated => return Err(AuthError::RefreshTokenReuseDetected),
            RefreshTokenStatus::Revoked => return Err(AuthError::RefreshTokenAlreadyRotated),
            RefreshTokenStatus::Active => {}
        }
        if self.is_expired(now) {
            return Err(AuthError::RefreshTokenExpired);
        }

        let successor = RefreshToken::issue(RefreshTokenIssueParams {
            session_id: self.session_id,
            account_id: self.account_id,
            token_hash: new_hash,
            issued_at: now,
            expires_at: new_expires_at,
        })?;

        self.status = RefreshTokenStatus::Rotated;
        self.used_at = Some(now);
        self.replaced_by = Some(successor.id);
        self.touch();
        Ok(successor)
    }

    /// Revokes an `Active` token (its session was signed out). Idempotent-safe at
    /// the application layer: already-terminal tokens return an error the caller
    /// can treat as a no-op.
    pub fn revoke(&mut self, now: DateTime<Utc>) -> Result<(), AuthError> {
        match self.status {
            RefreshTokenStatus::Active => {
                self.status = RefreshTokenStatus::Revoked;
                self.used_at = Some(now);
                self.touch();
                Ok(())
            }
            RefreshTokenStatus::Rotated => Err(AuthError::RefreshTokenAlreadyRotated),
            RefreshTokenStatus::Revoked => Err(AuthError::InvalidSessionTransition {
                from: "revoked".into(),
                to: "revoked".into(),
            }),
        }
    }

    fn touch(&mut self) {
        self.version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn hash(s: &str) -> RefreshTokenHash {
        RefreshTokenHash::new(s).unwrap()
    }

    fn issue() -> RefreshToken {
        RefreshToken::issue(RefreshTokenIssueParams {
            session_id: SessionId::new(),
            account_id: AccountId::from_uuid(Uuid::now_v7()),
            token_hash: hash("h0"),
            issued_at: t0(),
            expires_at: t0() + Duration::days(7),
        })
        .unwrap()
    }

    #[test]
    fn issue_starts_active() {
        let rt = issue();
        assert_eq!(rt.status(), RefreshTokenStatus::Active);
        assert!(rt.replaced_by().is_none());
    }

    #[test]
    fn issue_rejects_non_positive_lifetime() {
        let err = RefreshToken::issue(RefreshTokenIssueParams {
            session_id: SessionId::new(),
            account_id: AccountId::from_uuid(Uuid::now_v7()),
            token_hash: hash("h"),
            issued_at: t0(),
            expires_at: t0(),
        })
        .unwrap_err();
        assert!(matches!(err, AuthError::DomainViolation { .. }));
    }

    #[test]
    fn rotate_links_lineage_and_marks_rotated() {
        let mut rt = issue();
        let successor = rt.rotate(t0() + Duration::hours(1), hash("h1"), t0() + Duration::days(7)).unwrap();
        assert_eq!(rt.status(), RefreshTokenStatus::Rotated);
        assert_eq!(rt.replaced_by(), Some(successor.id()));
        assert_eq!(successor.status(), RefreshTokenStatus::Active);
        assert_eq!(successor.session_id(), rt.session_id());
        assert_eq!(successor.account_id(), rt.account_id());
    }

    #[test]
    fn reuse_of_rotated_token_is_detected() {
        let mut rt = issue();
        let _succ = rt.rotate(t0() + Duration::hours(1), hash("h1"), t0() + Duration::days(7)).unwrap();
        // Presenting the SAME (now-rotated) token again ⇒ reuse.
        let err = rt.rotate(t0() + Duration::hours(2), hash("h2"), t0() + Duration::days(7)).unwrap_err();
        assert!(matches!(err, AuthError::RefreshTokenReuseDetected));
    }

    #[test]
    fn rotate_fails_when_expired() {
        let mut rt = issue();
        let err = rt.rotate(t0() + Duration::days(8), hash("h1"), t0() + Duration::days(15)).unwrap_err();
        assert!(matches!(err, AuthError::RefreshTokenExpired));
        // unchanged
        assert_eq!(rt.status(), RefreshTokenStatus::Active);
    }

    #[test]
    fn rotate_of_revoked_token_reports_already_rotated() {
        let mut rt = issue();
        rt.revoke(t0() + Duration::hours(1)).unwrap();
        let err = rt.rotate(t0() + Duration::hours(2), hash("h1"), t0() + Duration::days(7)).unwrap_err();
        assert!(matches!(err, AuthError::RefreshTokenAlreadyRotated));
    }

    #[test]
    fn revoke_active_then_double_revoke_is_illegal() {
        let mut rt = issue();
        rt.revoke(t0() + Duration::hours(1)).unwrap();
        assert_eq!(rt.status(), RefreshTokenStatus::Revoked);
        assert!(matches!(
            rt.revoke(t0() + Duration::hours(2)).unwrap_err(),
            AuthError::InvalidSessionTransition { .. }
        ));
    }
}
