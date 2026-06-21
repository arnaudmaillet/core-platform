use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

/// Reason a profile was reactively masked without operator intervention.
///
/// Populated by the Kafka account event consumer. Kept separate from
/// `suspension_reason` (an admin-set free-text field) to distinguish
/// programmatic masking from manual moderation actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskingReason {
    AccountSuspended,
    AccountDeleted,
    ContentPolicyViolation,
}

impl MaskingReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccountSuspended       => "account_suspended",
            Self::AccountDeleted         => "account_deleted",
            Self::ContentPolicyViolation => "content_policy_violation",
        }
    }
}

impl fmt::Display for MaskingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for MaskingReason {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "account_suspended"        => Ok(Self::AccountSuspended),
            "account_deleted"          => Ok(Self::AccountDeleted),
            "content_policy_violation" => Ok(Self::ContentPolicyViolation),
            other => Err(ProfileError::DomainViolation {
                field: "masking_reason".into(),
                message: format!("unknown masking reason: '{other}'"),
            }),
        }
    }
}

impl TryFrom<String> for MaskingReason {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
