// crates/account/src/domain/value_objects/token.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::core::{DomainError, Result};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerificationToken(String);

impl VerificationToken {
    /// Crée une instance à partir d'une chaîne avec validation immédiate.
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let vo = Self(value.into());
        vo.validate()?;
        Ok(vo)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for VerificationToken {
    fn validate(&self) -> Result<()> {
        let val = self.0.trim();
        if val.is_empty() {
            return Err(DomainError::Validation {
                field: "token",
                reason: "Token cannot be empty".to_string(),
            });
        }
        if val.len() < 8 {
            return Err(DomainError::Validation {
                field: "token",
                reason: "Token is too short (min 8 chars)".to_string(),
            });
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl FromStr for VerificationToken {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl fmt::Display for VerificationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for VerificationToken {
    fn default() -> Self {
        // Utile pour les tests, mais attention à la validation
        Self("default_placeholder_token".to_string())
    }
}
