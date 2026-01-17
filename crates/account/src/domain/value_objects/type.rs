// crates/account/src/domain/value_objects/type

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Individual,
    Creator,
    Business,
    System,
}

impl AccountType {
    /// Constructeur sécurisé (API/Commandes)
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    pub fn new_unchecked(user_type: AccountType) -> Self {
        user_type
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Individual => "individual",
            Self::Creator => "creator",
            Self::Business => "business",
            Self::System => "system",
        }
    }

    // --- LOGIQUE MÉTIER ---

    pub fn is_human(&self) -> bool {
        !matches!(self, Self::System)
    }

    pub fn can_monetize(&self) -> bool {
        matches!(self, Self::Creator | Self::Business)
    }
}

impl ValueObject for AccountType {
    fn validate(&self) -> Result<()> {
        // L'énumération garantit la validité par construction
        Ok(())
    }
}

impl Default for AccountType {
    fn default() -> Self {
        Self::Individual
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountType {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        // Match performant sans allocation String::to_lowercase()
        match s.trim() {
            "individual" | "Individual" => Ok(Self::Individual),
            "creator" | "Creator" => Ok(Self::Creator),
            "business" | "Business" => Ok(Self::Business),
            "system" | "System" => Ok(Self::System),
            _ => Err(DomainError::Validation {
                field: "user_type",
                reason: format!("Unknown user type: {}", s),
            }),
        }
    }
}

impl TryFrom<String> for AccountType {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}