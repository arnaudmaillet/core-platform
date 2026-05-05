// crates/shared-kernel/src/domain/beta_tier
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BetaTier {
    NONE,
    BETA,
    ALPHA,
    INTERNAL,
}

impl BetaTier {
    /// Constructeur sécurisé pour les entrées externes (API/Commandes)
    pub fn try_new(value: &str) -> Result<Self> {
        Self::from_str(value)
    }

    /// Reconstruction rapide (Infrastructure/DB)
    pub fn from_raw(tier: BetaTier) -> Self {
        tier
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NONE => "NONE",
            Self::BETA => "BETA",
            Self::ALPHA => "ALPHA",
            Self::INTERNAL => "INTERNAL",
        }
    }

    // --- LOGIQUE MÉTIER ---

    /// Vérifie si l'utilisateur a accès aux fonctionnalités expérimentales
    pub fn has_experimental_access(&self) -> bool {
        !matches!(self, Self::NONE)
    }

    /// Détermine si l'utilisateur doit voir les logs de debug ou outils internes
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::INTERNAL)
    }

    /// Vérifie si le tier actuel est supérieur ou égal à un niveau requis
    /// (Utile pour une hiérarchie : INTERNAL > ALPHA > BETA > NONE)
    pub fn meets_requirement(&self, required: Self) -> bool {
        match (self, required) {
            (Self::INTERNAL, _) => true,
            (Self::ALPHA, Self::INTERNAL) => false,
            (Self::ALPHA, _) => true,
            (Self::BETA, Self::INTERNAL | Self::ALPHA) => false,
            (Self::BETA, _) => true,
            (Self::NONE, Self::NONE) => true,
            (Self::NONE, _) => false,
        }
    }
}

impl ValueObject for BetaTier {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for BetaTier {
    fn default() -> Self {
        Self::NONE
    }
}

// --- CONVERSIONS ---

impl FromStr for BetaTier {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_uppercase().as_str() {
            "NONE" => Ok(Self::NONE),
            "BETA" => Ok(Self::BETA),
            "ALPHA" => Ok(Self::ALPHA),
            "INTERNAL" => Ok(Self::INTERNAL),
            _ => Err(DomainError::Validation {
                field: "beta_tier",
                reason: format!("Unknown beta tier: {}", s),
            }),
        }
    }
}

impl TryFrom<String> for BetaTier {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl fmt::Display for BetaTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<i32> for BetaTier {
    type Error = String;

    fn try_from(value: i32) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NONE),
            1 => Ok(Self::BETA),
            2 => Ok(Self::ALPHA),
            3 => Ok(Self::INTERNAL),
            _ => Err(format!("'{}' is not a valid AccountRole ID", value)),
        }
    }
}
