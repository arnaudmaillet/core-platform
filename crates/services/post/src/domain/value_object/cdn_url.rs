use serde::{Deserialize, Serialize};
use crate::error::PostError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdnUrl(String);

impl CdnUrl {
    pub fn new(s: impl Into<String>) -> Result<Self, PostError> {
        let s = s.into();
        if !s.starts_with("https://") || s.len() <= 8 {
            return Err(PostError::InvalidCdnUrl { url: s });
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<CdnUrl> for String {
    fn from(u: CdnUrl) -> Self {
        u.0
    }
}
