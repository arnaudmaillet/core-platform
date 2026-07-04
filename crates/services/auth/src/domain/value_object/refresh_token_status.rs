use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// Lifecycle state of a single refresh-token row.
///
/// `Active` is non-terminal. A successful rotation moves `Active → Rotated`
/// (single-use); an explicit revoke moves `Active → Revoked`. Presenting a token
/// already in `Rotated` is the reuse-detection signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshTokenStatus {
    /// Valid and unused; may be rotated exactly once.
    Active,
    /// Already exchanged for a successor. Terminal. Re-presentation ⇒ reuse.
    Rotated,
    /// Invalidated (its session was revoked / signed out). Terminal.
    Revoked,
}

impl RefreshTokenStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Rotated => "rotated",
            Self::Revoked => "revoked",
        }
    }

    pub fn can_transition_to(&self, next: RefreshTokenStatus) -> bool {
        use RefreshTokenStatus::*;
        matches!((self, next), (Active, Rotated) | (Active, Revoked))
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Rotated | Self::Revoked)
    }
}

impl fmt::Display for RefreshTokenStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RefreshTokenStatus {
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(Self::Active),
            "rotated" => Ok(Self::Rotated),
            "revoked" => Ok(Self::Revoked),
            other => Err(AuthError::DomainViolation {
                field: "refresh_token_status".into(),
                message: format!("unknown refresh-token status: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions() {
        assert!(RefreshTokenStatus::Active.can_transition_to(RefreshTokenStatus::Rotated));
        assert!(RefreshTokenStatus::Active.can_transition_to(RefreshTokenStatus::Revoked));
        assert!(!RefreshTokenStatus::Rotated.can_transition_to(RefreshTokenStatus::Revoked));
        assert!(!RefreshTokenStatus::Revoked.can_transition_to(RefreshTokenStatus::Rotated));
    }
}
