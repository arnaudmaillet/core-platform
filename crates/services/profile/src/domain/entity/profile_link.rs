use serde::{Deserialize, Serialize};

use crate::domain::value_object::WebsiteUrl;
use crate::error::ProfileError;

/// A named hyperlink attached to a profile (max 5 per profile).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileLink {
    pub label: String,
    pub url: WebsiteUrl,
}

impl ProfileLink {
    pub fn new(label: String, url: WebsiteUrl) -> Result<Self, ProfileError> {
        let trimmed = label.trim().to_owned();
        if trimmed.is_empty() {
            return Err(ProfileError::DomainViolation {
                field: "link.label".into(),
                message: "link label must not be blank".into(),
            });
        }
        if trimmed.len() > 32 {
            return Err(ProfileError::DomainViolation {
                field: "link.label".into(),
                message: format!(
                    "link label exceeds 32 characters (got {})",
                    trimmed.len()
                ),
            });
        }
        Ok(Self { label: trimmed, url })
    }
}
