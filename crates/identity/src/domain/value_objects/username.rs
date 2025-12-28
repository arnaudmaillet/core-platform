// crates/identity/src/value_objects/username.rs

use serde::{Deserialize, Serialize};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Username(String);

impl Username {
    pub fn new(username: &str) -> crate::Result<Self> {
        let username = username.trim().to_lowercase();

        if username.len() < 3 || username.len() > 30 {
            return Err(DomainError::Validation {
                field: "username",
                reason: "Length must be 3-30 characters".into(),
            });
        }

        if !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')  {
            return Err(DomainError::Validation {
                field: "username",
                reason: "Only alphanumeric, _ and - allowed".into(),
            });
        }

        if username.starts_with('_') || username.ends_with('_')   {
            return Err(DomainError::Validation {
                field: "username",
                reason: "Cannot start or end with _".into(),
            });
        }

        Ok(Self(username))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ToString for Username {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}