use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

/// Public identity category for a profile. Immutable after creation.
///
/// Immutability is enforced at the aggregate level: changing a profile's kind
/// requires creating a new profile under the same account (1-to-N ownership).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Personal,
    Professional,
    Brand,
    Bot,
}

impl ProfileKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Personal     => "personal",
            Self::Professional => "professional",
            Self::Brand        => "brand",
            Self::Bot          => "bot",
        }
    }
}

impl fmt::Display for ProfileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for ProfileKind {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "personal"     => Ok(Self::Personal),
            "professional" => Ok(Self::Professional),
            "brand"        => Ok(Self::Brand),
            "bot"          => Ok(Self::Bot),
            other => Err(ProfileError::InvalidProfileKind(other.to_owned())),
        }
    }
}

impl TryFrom<String> for ProfileKind {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
