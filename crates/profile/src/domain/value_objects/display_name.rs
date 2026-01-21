use serde::{Deserialize, Serialize};
use std::fmt;
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayName(String);

impl DisplayName {
    pub const MIN_LENGTH: usize = 1;
    pub const MAX_LENGTH: usize = 50;

    /// Constructeur sécurisé (API / Mise à jour profil)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Nettoyage initial : retrait des caractères de contrôle (sécurité UI)
        let cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control())
            .collect();

        // 2. Normalisation des espaces (évite les doubles espaces ou espaces de fin)
        let normalized = cleaned
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let display_name = Self(normalized);
        display_name.validate()?;
        Ok(display_name)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for DisplayName {
    fn validate(&self) -> Result<()> {
        let count = self.0.chars().count();

        if count < Self::MIN_LENGTH {
            return Err(DomainError::Validation {
                field: "display_name",
                reason: "Display name cannot be empty".into(),
            });
        }

        if count > Self::MAX_LENGTH {
            return Err(DomainError::Validation {
                field: "display_name",
                reason: format!("Display name too long (max {})", Self::MAX_LENGTH),
            });
        }

        Ok(())
    }
}

// --- CONVERTISSEURS ---

impl Default for DisplayName {
    fn default() -> Self {
        Self("New User".to_string())
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DisplayName {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<DisplayName> for String {
    fn from(dn: DisplayName) -> Self {
        dn.0
    }
}