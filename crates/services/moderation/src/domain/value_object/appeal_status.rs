use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// Lifecycle state of an [`Appeal`](crate::domain::aggregate::Appeal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppealStatus {
    Filed,
    UnderReview,
    /// The original decision stands. Terminal.
    Upheld,
    /// The decision is overturned; a reversal decision is recorded. Terminal.
    Overturned,
}

impl AppealStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filed => "filed",
            Self::UnderReview => "under_review",
            Self::Upheld => "upheld",
            Self::Overturned => "overturned",
        }
    }

    pub fn can_transition_to(&self, next: AppealStatus) -> bool {
        use AppealStatus::*;
        matches!(
            (self, next),
            (Filed, UnderReview)
                | (Filed, Upheld)
                | (Filed, Overturned)
                | (UnderReview, Upheld)
                | (UnderReview, Overturned)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Upheld | Self::Overturned)
    }
}

impl fmt::Display for AppealStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for AppealStatus {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "filed" => Ok(Self::Filed),
            "under_review" => Ok(Self::UnderReview),
            "upheld" => Ok(Self::Upheld),
            "overturned" => Ok(Self::Overturned),
            other => Err(ModerationError::DomainViolation {
                field: "appeal_status".into(),
                message: format!("unknown appeal status: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions() {
        assert!(AppealStatus::Filed.can_transition_to(AppealStatus::UnderReview));
        assert!(AppealStatus::Filed.can_transition_to(AppealStatus::Overturned));
        assert!(AppealStatus::UnderReview.can_transition_to(AppealStatus::Upheld));
        for from in [AppealStatus::Upheld, AppealStatus::Overturned] {
            assert!(!from.can_transition_to(AppealStatus::UnderReview));
        }
    }

    #[test]
    fn string_round_trip() {
        for s in [
            AppealStatus::Filed,
            AppealStatus::UnderReview,
            AppealStatus::Upheld,
            AppealStatus::Overturned,
        ] {
            assert_eq!(AppealStatus::try_from(s.as_str()).unwrap(), s);
        }
        assert!(AppealStatus::try_from("bogus").is_err());
    }
}
