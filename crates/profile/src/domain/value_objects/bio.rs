// crates/shared_kernel/src/domain/value_objects/bio.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct Bio(String);

impl Bio {
    pub const MAX_LENGTH: usize = 255;

    /// Constructeur sécurisé (API / Mise à jour profil)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(DomainError::Validation {
                field: "bio",
                reason: "Bio cannot be empty. Use Option::None for no bio.".into(),
            });
        }

        // Normalisation des sauts de ligne avant validation de longueur
        let normalized = Self::normalize_newlines(trimmed);

        let bio = Self(normalized);
        bio.validate()?;
        Ok(bio)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Nettoyage performant des sauts de ligne consécutifs
    /// On limite à 2 sauts maximum pour garder un affichage propre.
    fn normalize_newlines(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut newline_count = 0;
        for c in input.chars() {
            if c == '\n' || c == '\r' {
                newline_count += 1;
                if newline_count <= 2 {
                    result.push('\n');
                }
            } else {
                newline_count = 0;
                result.push(c);
            }
        }
        result.trim().to_string()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for Bio {
    fn validate(&self) -> Result<()> {
        // On compte les caractères Unicode réels, pas les octets
        let count = self.0.chars().count();

        if count > Self::MAX_LENGTH {
            return Err(DomainError::Validation {
                field: "bio",
                reason: format!(
                    "Bio is too long (max {} chars, got {})",
                    Self::MAX_LENGTH,
                    count
                ),
            });
        }

        // Sécurité : Interdire les caractères de contrôle non autorisés (hors sauts de ligne)
        if self.0.chars().any(|c| c.is_control() && c != '\n') {
            return Err(DomainError::Validation {
                field: "bio",
                reason: "Bio contains invalid control characters".into(),
            });
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for Bio {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<Bio> for String {
    fn from(bio: Bio) -> Self {
        bio.0
    }
}

impl fmt::Display for Bio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
