use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// Know-Your-Customer verification state.
///
/// Drives age-gating, compliance checks, and feature availability.
/// Transitions are enforced by [`KycStatus::can_transition_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KycStatus {
    /// No KYC documents submitted yet.
    NotStarted,
    /// Documents submitted; awaiting review.
    Submitted,
    /// Under active review by a compliance officer.
    InReview,
    /// KYC approved — full platform access granted.
    Approved,
    /// KYC rejected. The account holder may re-submit (Rejected → Submitted).
    Rejected,
}

impl KycStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Submitted  => "submitted",
            Self::InReview   => "in_review",
            Self::Approved   => "approved",
            Self::Rejected   => "rejected",
        }
    }

    /// Returns `true` if the transition `self → next` is a valid KYC
    /// progression. `Approved` is terminal; `Rejected` allows re-submission.
    pub fn can_transition_to(&self, next: KycStatus) -> bool {
        use KycStatus::*;
        matches!(
            (self, next),
            (NotStarted, Submitted)
                | (Submitted, InReview)
                | (InReview, Approved)
                | (InReview, Rejected)
                | (Rejected, Submitted)
        )
    }
}

impl fmt::Display for KycStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for KycStatus {
    type Error = AccountError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "not_started" => Ok(Self::NotStarted),
            "submitted"   => Ok(Self::Submitted),
            "in_review"   => Ok(Self::InReview),
            "approved"    => Ok(Self::Approved),
            "rejected"    => Ok(Self::Rejected),
            other => Err(AccountError::InvalidKycStatus(other.to_owned())),
        }
    }
}

impl TryFrom<String> for KycStatus {
    type Error = AccountError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
