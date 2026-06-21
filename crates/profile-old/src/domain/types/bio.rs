// crates/profile/src/domain/types/bio.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
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
            return Err(Error::validation(
                "bio",
                "Bio cannot be empty. Use Option::None for no bio.",
            ));
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
            return Err(Error::validation(
                "bio",
                format!(
                    "Bio is too long (max {} chars, got {})",
                    Self::MAX_LENGTH,
                    count
                ),
            ));
        }

        // Sécurité : Interdire les caractères de contrôle non autorisés (hors sauts de ligne)
        if self.0.chars().any(|c| c.is_control() && c != '\n') {
            return Err(Error::validation(
                "bio",
                "Bio contains invalid control characters",
            ));
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for Bio {
    type Error = Error;
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
