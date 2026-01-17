// crates/shared_kernel/src/domain/value_objects/location_label.rs

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocationLabel(String);

impl LocationLabel {
    pub const MIN_LENGTH: usize = 2;
    pub const MAX_LENGTH: usize = 100;

    /// Constructeur sécurisé (Domaine / API)
    /// Effectue le nettoyage et la normalisation avant validation
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Transformation / Normalisation
        // - Supprime les caractères de contrôle
        // - Normalise les espaces multiples ("Paris   France" -> "Paris France")
        let normalized = raw
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let label = Self(normalized);

        // 2. Validation
        label.validate()?;

        Ok(label)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for LocationLabel {
    fn validate(&self) -> Result<()> {
        let count = self.0.chars().count();

        if count < Self::MIN_LENGTH {
            return Err(DomainError::Validation {
                field: "location_label",
                reason: format!("Location too short (min {})", Self::MIN_LENGTH),
            });
        }

        if count > Self::MAX_LENGTH {
            return Err(DomainError::Validation {
                field: "location_label",
                reason: format!("Location too long (max {})", Self::MAX_LENGTH),
            });
        }

        Ok(())
    }
}

impl fmt::Display for LocationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Conversions ---

impl FromStr for LocationLabel {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for LocationLabel {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}