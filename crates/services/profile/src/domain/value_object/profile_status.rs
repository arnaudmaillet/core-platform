use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

/// Lifecycle state of a profile.
///
/// Transitions are governed by [`ProfileStatus::can_transition_to`].
/// `Deleted` is a terminal state; no transitions out are permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    Active,
    Suspended,
    Hidden,
    Deleted,
}

impl ProfileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active    => "active",
            Self::Suspended => "suspended",
            Self::Hidden    => "hidden",
            Self::Deleted   => "deleted",
        }
    }

    pub fn can_transition_to(&self, next: ProfileStatus) -> bool {
        use ProfileStatus::*;
        matches!(
            (self, next),
            (Active, Suspended)
                | (Active, Hidden)
                | (Active, Deleted)
                | (Suspended, Active)
                | (Suspended, Hidden)
                | (Suspended, Deleted)
                | (Hidden, Active)
                | (Hidden, Suspended)
                | (Hidden, Deleted)
        )
    }
}

impl fmt::Display for ProfileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for ProfileStatus {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active"    => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "hidden"    => Ok(Self::Hidden),
            "deleted"   => Ok(Self::Deleted),
            other => Err(ProfileError::InvalidProfileStatus(other.to_owned())),
        }
    }
}

impl TryFrom<String> for ProfileStatus {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
