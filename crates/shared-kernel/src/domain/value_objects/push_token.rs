use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PushToken(String);

impl PushToken {
    /// Longueur minimale arbitraire pour éviter les faux tokens (ex: "null", "undefined")
    pub const MIN_LENGTH: usize = 8;
    /// Limite haute pour éviter les attaques par déni de service (DoS) mémoire
    pub const MAX_LENGTH: usize = 1024;

    /// Constructeur sécurisé (Domaine / API)
    pub fn try_new(token: impl Into<String>) -> Result<Self> {
        let raw = token.into();
        let trimmed = raw.trim().to_string();

        let push_token = Self(trimmed);
        push_token.validate()?;
        Ok(push_token)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn new_unchecked(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for PushToken {
    fn validate(&self) -> Result<()> {
        let len = self.0.len();

        if len < Self::MIN_LENGTH {
            return Err(DomainError::Validation {
                field: "push_token",
                reason: format!("Push token is abnormally short (min {})", Self::MIN_LENGTH),
            });
        }

        if len > Self::MAX_LENGTH {
            return Err(DomainError::Validation {
                field: "push_token",
                reason: format!("Push token is too long (max {})", Self::MAX_LENGTH),
            });
        }

        // On vérifie que la chaîne ne contient pas de caractères de contrôle
        if self.0.chars().any(|c| c.is_control()) {
            return Err(DomainError::Validation {
                field: "push_token",
                reason: "Push token contains invalid control characters".into(),
            });
        }

        Ok(())
    }
}

impl fmt::Display for PushToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Conversions ---

impl TryFrom<String> for PushToken {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}