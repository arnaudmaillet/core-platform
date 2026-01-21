// crates/account/src/domain/value_objects/locale.rs

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Locale(String);

impl Locale {
    /// Constructeur sécurisé (API / Inscription)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // Normalisation de base : trim et remplacement des '_' par des '-'
        // car certains SDK (Android/iOS) utilisent parfois l'underscore.
        let normalized = raw.trim().replace('_', "-");

        let locale = Self(normalized);
        locale.validate()?;
        Ok(locale)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extrait uniquement la langue (ex: "fr-FR" -> "fr")
    pub fn language_code(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }
}

impl ValueObject for Locale {
    fn validate(&self) -> Result<()> {
        let len = self.0.len();

        // Validation BCP-47 légère : min 2 (ex: "en"), max 10 (ex: "zh-Hans-CN")
        if len < 2 || len > 10 {
            return Err(DomainError::Validation {
                field: "locale",
                reason: format!("Locale length invalid ({}). Expected BCP-47 format.", self.0),
            });
        }

        // On vérifie que la chaîne ne contient que des caractères alphanumériques et des tirets
        if !self.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(DomainError::Validation {
                field: "locale",
                reason: "Locale contains invalid characters".into(),
            });
        }

        Ok(())
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self("en-US".to_string())
    }
}

// --- CONVERSIONS ---

impl FromStr for Locale {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for Locale {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<Locale> for String {
    fn from(locale: Locale) -> Self {
        locale.0
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}