use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::value_object::avatar_url::validate_https_url;
use crate::error::ProfileError;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WebsiteUrl(String);

impl WebsiteUrl {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        validate_https_url(raw)?;
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WebsiteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WebsiteUrl({})", self.0)
    }
}

impl fmt::Display for WebsiteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
