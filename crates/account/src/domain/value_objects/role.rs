// crates/account/src/domain/value_objects/ole

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::core::{DomainError, Result};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AccountRole {
    USER = 0,
    MODERATOR = 10,
    STAFF = 20,
    ADMIN = 30,
}

impl AccountRole {
    /// Constructeur sécurisé
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    /// Ici, comme c'est un Enum Copy, from_raw est souvent identique à un mapping direct
    pub fn from_raw(role: AccountRole) -> Self {
        role
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::USER => "USER",
            Self::MODERATOR => "MODERATOR",
            Self::STAFF => "STAFF",
            Self::ADMIN => "ADMIN",
        }
    }

    // --- LOGIQUE DE PERMISSIONS ---

    pub fn has_permission_of(&self, other: AccountRole) -> bool {
        *self >= other
    }

    pub fn can_suspend(&self) -> bool {
        self.has_permission_of(Self::MODERATOR)
    }

    pub fn can_access_admin_panel(&self) -> bool {
        self.has_permission_of(Self::STAFF)
    }

    pub fn as_lowercase(&self) -> &'static str {
        match self {
            Self::USER => "user",
            Self::MODERATOR => "moderator",
            Self::STAFF => "staff",
            Self::ADMIN => "admin",
        }
    }
}

impl ValueObject for AccountRole {
    fn validate(&self) -> Result<()> {
        // L'enum garantit par construction la validité des variantes
        Ok(())
    }
}

impl Default for AccountRole {
    fn default() -> Self {
        Self::USER
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountRole {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().trim() {
            "USER" => Ok(Self::USER),
            "MODERATOR" => Ok(Self::MODERATOR),
            "STAFF" => Ok(Self::STAFF),
            "ADMIN" => Ok(Self::ADMIN),
            _ => Err(DomainError::Validation {
                field: "role",
                reason: format!("Unknown account role: {}", s),
            }),
        }
    }
}

impl TryFrom<String> for AccountRole {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<i32> for AccountRole {
    type Error = String;

    fn try_from(value: i32) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::USER),
            1 => Ok(Self::STAFF),
            2 => Ok(Self::ADMIN),
            _ => Err(format!("'{}' is not a valid AccountRole ID", value)),
        }
    }
}

impl fmt::Display for AccountRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
