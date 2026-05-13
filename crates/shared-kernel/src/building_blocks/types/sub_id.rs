// crates/account/src/domain/value_objects/sub_id.rs

// crates/account/src/domain/value_objects/locale.rs

use crate::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubId(String);

impl SubId {
    /// Constructeur sécurisé (API / Auth Provider Callback)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();
        let cleaned = raw.trim().to_string();

        let id = Self(cleaned);
        id.validate()?;
        Ok(id)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for SubId {
    fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::validation(
                "sub_id",
                "Sub provider ID cannot be empty",
            ));
        }

        // Sécurité : On limite la taille pour éviter les injections de payloads massifs
        if self.0.len() > 128 {
            return Err(Error::validation("sub_id", "Sub ID is suspiciously long"));
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl FromStr for SubId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for SubId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<SubId> for String {
    fn from(id: SubId) -> Self {
        id.0
    }
}

impl fmt::Display for SubId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
