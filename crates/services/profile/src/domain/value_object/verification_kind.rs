use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    Official,
    Notable,
    Business,
}

impl VerificationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Notable  => "notable",
            Self::Business => "business",
        }
    }
}

impl fmt::Display for VerificationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for VerificationKind {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "official" => Ok(Self::Official),
            "notable"  => Ok(Self::Notable),
            "business" => Ok(Self::Business),
            other => Err(ProfileError::DomainViolation {
                field: "verification_kind".into(),
                message: format!("unknown verification kind: '{other}'"),
            }),
        }
    }
}

impl TryFrom<String> for VerificationKind {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
