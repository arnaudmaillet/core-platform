// crates/shared-kernel/src/building_blocks/types/region.rs

use crate::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Énumération stricte des régions physiques de l'infrastructure (Shards globaux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegionCode {
    EU,
    US,
    ASIA,
}

/// Value Object encapsulant la région.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Region(RegionCode);

impl Region {
    /// Constructeur sécurisé depuis n'importe quelle chaîne textuelle.
    /// Gère la normalisation (majuscules, espaces).
    pub fn try_new(code: impl AsRef<str>) -> Result<Self> {
        let normalized = code.as_ref().trim().to_uppercase();

        let region = match normalized.as_str() {
            "EU" => RegionCode::EU,
            "US" => RegionCode::US,
            "ASIA" => RegionCode::ASIA,
            _ => {
                return Err(Error::validation(
                    "region",
                    format!(
                        "Region '{}' is not supported. Valid regions: EU, US, ASIA",
                        normalized
                    ),
                ));
            }
        };

        Ok(Self(region))
    }

    pub fn from_raw(region: RegionCode) -> Self {
        Self(region)
    }

    pub fn inner(&self) -> RegionCode {
        self.0
    }

    /// Expose la région sous forme de tranche de chaîne (`&str`).
    pub fn as_str(&self) -> &str {
        self.as_static_str()
    }

    /// 💡 LA MÉTHODE MAGIQUE AUTOMATIQUE
    /// Retourne une référence de chaîne de caractères garantie statique.
    /// Résout à 100% les bugs de lifetimes dans les architectures d'événements.
    pub fn as_static_str(&self) -> &'static str {
        match self.0 {
            RegionCode::EU => "EU",
            RegionCode::US => "US",
            RegionCode::ASIA => "ASIA",
        }
    }
}

impl ValueObject for Region {
    /// Toujours valide par construction grâce au typage de l'enum Rust.
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for Region {
    /// Région historique / Shard principal par défaut
    fn default() -> Self {
        Self(RegionCode::EU)
    }
}

// --- CONVERSIONS POUR LE FRAMEWORK ---

impl FromStr for Region {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for Region {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::try_new(&value)
    }
}

impl TryFrom<&str> for Region {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::try_new(value)
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_static_str())
    }
}
