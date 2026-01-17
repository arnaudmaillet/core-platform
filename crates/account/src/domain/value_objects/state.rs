// crates/shared-kernel/src/domain/account_state
use std::fmt;
use std::str::FromStr;
use serde::{Serialize, Deserialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AccountState {
    Pending,
    Active,
    Deactivated,
    Suspended,
    Banned,
}

impl AccountState {
    /// Constructeur sécurisé (API/Commandes)
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    pub fn new_unchecked(state: AccountState) -> Self {
        state
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Deactivated => "deactivated",
            Self::Suspended => "suspended",
            Self::Banned => "banned"
        }
    }

    // --- LOGIQUE MÉTIER ---

    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Deactivated | Self::Suspended | Self::Banned)
    }

    pub fn can_authenticate(&self) -> bool {
        matches!(self, Self::Active | Self::Pending)
    }
}

impl ValueObject for AccountState {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::Pending
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountState {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        // Match performant sans allocation String
        match s.trim().to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "deactivated" => Ok(Self::Deactivated),
            "suspended" => Ok(Self::Suspended),
            "banned" => Ok(Self::Banned),
            _ => Err(DomainError::Validation {
                field: "account_state",
                reason: format!("Unknown account state: {}", s),
            }),
        }
    }
}

impl TryFrom<String> for AccountState {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl fmt::Display for AccountState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}