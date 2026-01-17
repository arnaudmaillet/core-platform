// crates/shared_kernel/src/domain/value_objects/timezone.rs

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Timezone(String);

impl Timezone {
    /// Constructeur sécurisé avec validation IANA
    pub fn try_new(tz: impl Into<String>) -> Result<Self> {
        let tz_str = tz.into().trim().to_string(); // On nettoie les espaces
        let timezone = Self(tz_str);
        timezone.validate()?;
        Ok(timezone)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    pub fn new_unchecked(tz: impl Into<String>) -> Self {
        Self(tz.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convertit vers l'énumération forte de chrono_tz pour les calculs de dates
    pub fn to_tz(&self) -> chrono_tz::Tz {
        self.0.parse::<chrono_tz::Tz>()
            .expect("Corrupted Timezone: Must be validated at construction")
    }
}

impl ValueObject for Timezone {
    fn validate(&self) -> Result<()> {
        // La validation IANA est coûteuse (parsing de table), 
        // on ne l'appelle qu'à la création ou via validate().
        if self.0.parse::<chrono_tz::Tz>().is_err() {
            return Err(DomainError::Validation {
                field: "timezone",
                reason: format!("'{}' is not a valid IANA timezone (ex: 'Europe/Paris')", self.0),
            });
        }
        Ok(())
    }
}

impl Default for Timezone {
    fn default() -> Self {
        Self("UTC".to_string())
    }
}

impl fmt::Display for Timezone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for Timezone {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}