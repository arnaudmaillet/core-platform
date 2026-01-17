// crates/account/src/domain/value_objects/ole

use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountRole {
    User = 0,
    Moderator = 10,
    Staff = 20,
    Admin = 30,
}

impl AccountRole {
    /// Constructeur sécurisé
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    /// Ici, comme c'est un Enum Copy, new_unchecked est souvent identique à un mapping direct
    pub fn new_unchecked(role: AccountRole) -> Self {
        role
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Moderator => "moderator",
            Self::Staff => "staff",
            Self::Admin => "admin",
        }
    }

    // --- LOGIQUE DE PERMISSIONS ---

    pub fn has_permission_of(&self, other: AccountRole) -> bool {
        *self >= other
    }

    pub fn can_suspend(&self) -> bool {
        self.has_permission_of(Self::Moderator)
    }

    pub fn can_access_admin_panel(&self) -> bool {
        self.has_permission_of(Self::Staff)
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
        Self::User
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountRole {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        // Hyperscale: On compare des &str sans to_lowercase() si possible
        // ou on gère le trim de manière performante.
        match s.trim() {
            "user" | "User" => Ok(Self::User),
            "moderator" | "Moderator" => Ok(Self::Moderator),
            "staff" | "Staff" => Ok(Self::Staff),
            "admin" | "Admin" => Ok(Self::Admin),
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

impl fmt::Display for AccountRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}