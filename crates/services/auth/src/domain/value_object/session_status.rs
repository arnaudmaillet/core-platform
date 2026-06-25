use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AuthError;

/// Lifecycle state of a [`Session`](crate::domain::aggregate::Session).
///
/// `Active` is the only non-terminal state. Transitions are gated by
/// [`SessionStatus::can_transition_to`]; the aggregate rejects any illegal
/// transition before mutating state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Live; may mint edge tokens and be extended until its expiry.
    Active,
    /// Explicitly revoked (logout / reuse-detection / admin). Terminal.
    Revoked,
    /// Reached its absolute expiry. Terminal.
    Expired,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    /// Returns `true` if `self → next` is a permitted transition.
    pub fn can_transition_to(&self, next: SessionStatus) -> bool {
        use SessionStatus::*;
        matches!(
            (self, next),
            (Active, Revoked) | (Active, Expired)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Revoked | Self::Expired)
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for SessionStatus {
    type Error = AuthError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            "expired" => Ok(Self::Expired),
            other => Err(AuthError::DomainViolation {
                field: "session_status".into(),
                message: format!("unknown session status: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_active_can_transition() {
        assert!(SessionStatus::Active.can_transition_to(SessionStatus::Revoked));
        assert!(SessionStatus::Active.can_transition_to(SessionStatus::Expired));
        // terminal states never transition
        for from in [SessionStatus::Revoked, SessionStatus::Expired] {
            for to in [SessionStatus::Active, SessionStatus::Revoked, SessionStatus::Expired] {
                assert!(!from.can_transition_to(to), "{from} -> {to} must be illegal");
            }
        }
        // Active cannot go back to Active
        assert!(!SessionStatus::Active.can_transition_to(SessionStatus::Active));
    }

    #[test]
    fn string_round_trip() {
        for s in [SessionStatus::Active, SessionStatus::Revoked, SessionStatus::Expired] {
            assert_eq!(SessionStatus::try_from(s.as_str()).unwrap(), s);
        }
        assert!(SessionStatus::try_from("bogus").is_err());
    }
}
