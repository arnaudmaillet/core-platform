// crates/identity/src/value_objects/bio.rs

use crate::{DomainError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayName(String);

impl DisplayName {
    pub fn new(name: &str) -> Result<Self> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::Validation {
                field: "display_name",
                reason: "cannot be empty".into(),
            });
        }
        if name.len() > 50 {
            return Err(DomainError::Validation {
                field: "display_name",
                reason: "maximum 50 characters".into(),
            });
        }
        Ok(Self(name.to_string()))
    }
}