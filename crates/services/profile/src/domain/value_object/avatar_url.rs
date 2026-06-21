use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AvatarUrl(String);

impl AvatarUrl {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        validate_https_url(raw)?;
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AvatarUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AvatarUrl({})", self.0)
    }
}

impl fmt::Display for AvatarUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn validate_https_url(raw: &str) -> Result<(), ProfileError> {
    if raw.len() > 2048 {
        return Err(ProfileError::InvalidUrl(format!(
            "URL exceeds 2048 characters (got {})",
            raw.len()
        )));
    }
    if !raw.starts_with("https://") {
        return Err(ProfileError::InvalidUrl(
            "URL must use the https scheme".into(),
        ));
    }
    Ok(())
}
