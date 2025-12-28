// crates/identity/src/value_objects/email.rs

use crate::{DomainError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bio(String);

impl Bio {
    pub fn new(bio: &str) -> Result<Self> {
        let bio = bio.trim();
        if bio.len() > 500 {
            return Err(DomainError::Validation {
                field: "bio",
                reason: "maximum 500 characters".into(),
            });
        }
        Ok(Self(bio.to_string()))
    }
}