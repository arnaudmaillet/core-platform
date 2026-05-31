// crates/shared-kernel/src/domain/state
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AccountState {
    PENDING,
    UNVERIFIED,
    ACTIVE,
    DEACTIVATED,
    SUSPENDED,
    BANNED,
}

impl AccountState {
    /// Constructeur sécurisé (API/Commandes)
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    pub fn from_raw(state: AccountState) -> Self {
        state
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PENDING => "PENDING",
            Self::UNVERIFIED => "UNVERIFIED",
            Self::ACTIVE => "ACTIVE",
            Self::DEACTIVATED => "DEACTIVATED",
            Self::SUSPENDED => "SUSPENDED",
            Self::BANNED => "BANNED",
        }
    }

    // --- LOGIQUE MÉTIER ---

    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::DEACTIVATED | Self::SUSPENDED | Self::BANNED)
    }

    pub fn can_authenticate(&self) -> bool {
        matches!(self, Self::ACTIVE | Self::UNVERIFIED)
    }
}

impl ValueObject for AccountState {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::UNVERIFIED
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_uppercase().as_str() {
            "PENDING" => Ok(Self::PENDING),
            "UNVERFIED" => Ok(Self::UNVERIFIED),
            "ACTIVE" => Ok(Self::ACTIVE),
            "DEACTIVATED" => Ok(Self::DEACTIVATED),
            "SUSPENDED" => Ok(Self::SUSPENDED),
            "BANNED" => Ok(Self::BANNED),
            _ => Err(Error::validation(
                "account_state",
                format!("Unknown account state: {}", s),
            )),
        }
    }
}

impl TryFrom<String> for AccountState {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl fmt::Display for AccountState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
