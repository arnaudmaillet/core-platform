use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileVisibility {
    Public,
    Private,
}

impl ProfileVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public  => "public",
            Self::Private => "private",
        }
    }
}

impl fmt::Display for ProfileVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for ProfileVisibility {
    type Error = ProfileError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "public"  => Ok(Self::Public),
            "private" => Ok(Self::Private),
            other => Err(ProfileError::DomainViolation {
                field: "visibility".into(),
                message: format!("unknown visibility: '{other}'"),
            }),
        }
    }
}

impl TryFrom<String> for ProfileVisibility {
    type Error = ProfileError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
