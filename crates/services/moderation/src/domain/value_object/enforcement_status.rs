use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// Lifecycle state of an
/// [`EnforcementAction`](crate::domain::aggregate::EnforcementAction). `Active` is
/// the only non-terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementStatus {
    Active,
    /// Reached its time-boxed expiry. Terminal.
    Expired,
    /// Lifted by a reversal (appeal overturn / re-review). Terminal.
    Reversed,
}

impl EnforcementStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Reversed => "reversed",
        }
    }

    pub fn can_transition_to(&self, next: EnforcementStatus) -> bool {
        use EnforcementStatus::*;
        matches!((self, next), (Active, Expired) | (Active, Reversed))
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Expired | Self::Reversed)
    }
}

impl fmt::Display for EnforcementStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for EnforcementStatus {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "reversed" => Ok(Self::Reversed),
            other => Err(ModerationError::DomainViolation {
                field: "enforcement_status".into(),
                message: format!("unknown enforcement status: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_active_transitions() {
        assert!(EnforcementStatus::Active.can_transition_to(EnforcementStatus::Expired));
        assert!(EnforcementStatus::Active.can_transition_to(EnforcementStatus::Reversed));
        for from in [EnforcementStatus::Expired, EnforcementStatus::Reversed] {
            for to in [
                EnforcementStatus::Active,
                EnforcementStatus::Expired,
                EnforcementStatus::Reversed,
            ] {
                assert!(!from.can_transition_to(to));
            }
        }
    }

    #[test]
    fn string_round_trip() {
        for s in [
            EnforcementStatus::Active,
            EnforcementStatus::Expired,
            EnforcementStatus::Reversed,
        ] {
            assert_eq!(EnforcementStatus::try_from(s.as_str()).unwrap(), s);
        }
        assert!(EnforcementStatus::try_from("bogus").is_err());
    }
}
