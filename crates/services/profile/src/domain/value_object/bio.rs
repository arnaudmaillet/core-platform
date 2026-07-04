use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bio(String);

impl Bio {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        if raw.len() > 500 {
            return Err(ProfileError::InvalidBio(format!(
                "bio exceeds 500 characters (got {})",
                raw.len()
            )));
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Bio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bio({})", self.0)
    }
}

impl fmt::Display for Bio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
