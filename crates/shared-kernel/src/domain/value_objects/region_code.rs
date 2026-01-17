// crates/shared-kernel/src/domain/region_code

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionCode(String);

impl RegionCode {
    pub const EU: &'static str = "eu";
    pub const US: &'static str = "us";
    pub const ASIA: &'static str = "asia";

    /// Constructeur sécurisé : normalise en minuscules et valide
    pub fn try_new(code: impl Into<String>) -> Result<Self> {
        let code_raw = code.into().to_lowercase().trim().to_string();
        let region = Self(code_raw);
        region.validate()?;
        Ok(region)
    }

    /// Reconstruction ultra-rapide pour l'infrastructure (DB/Cache)
    pub fn new_unchecked(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for RegionCode {
    fn validate(&self) -> Result<()> {
        match self.0.as_str() {
            Self::EU | Self::US | Self::ASIA => Ok(()),
            _ => Err(DomainError::Validation {
                field: "region_code",
                reason: format!(
                    "Region '{}' not supported. Valid: {}, {}, {}",
                    self.0, Self::EU, Self::US, Self::ASIA
                ),
            }),
        }
    }
}

impl Default for RegionCode {
    fn default() -> Self {
        Self(Self::EU.to_string())
    }
}

// --- CONVERSIONS ---

impl FromStr for RegionCode {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for RegionCode {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl fmt::Display for RegionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}