use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DisplayName(String);

impl DisplayName {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ProfileError::InvalidDisplayName(
                "display name must not be blank".into(),
            ));
        }
        if trimmed.len() > 100 {
            return Err(ProfileError::InvalidDisplayName(format!(
                "display name exceeds 100 characters (got {})",
                trimmed.len()
            )));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayName({})", self.0)
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
