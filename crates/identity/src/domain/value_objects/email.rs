// crates/identity/src/value_objects/email.rs

use serde::{Deserialize, Serialize};
use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    pub fn new(email: &str) -> crate::Result<Self> {
        let email = email.trim().to_lowercase();

        if !email.contains('@') || email.len() > 254 {
            return Err(DomainError::Validation {
                field: "email",
                reason: "Invalid format".into(),
            });
        }

        // Validation plus strict a ajouter
        Ok(Self(email))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}