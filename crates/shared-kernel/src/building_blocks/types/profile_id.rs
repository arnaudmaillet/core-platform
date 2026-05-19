// crates/profile/src/domain/types/profile_id.rs

use serde::{Deserialize, Serialize};
use crate::core::{Error, Identifier, Result, ValueObject};
use crate::types::Region;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique "Smart" pour un profil, basé sur l'UUID v7.
/// Auto-porte sa région géographique directement codée dans ses 128 bits binaires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    /// Génère un nouvel identifiant unique (UUID v7) en y injectant dynamiquement la région.
    pub fn generate(region: Region) -> Self {
        // 1. Génération d'un UUID v7 standard (contient le timestamp actuel)
        let mut bytes = Uuid::now_v7().into_bytes();

        // 2. Injection de la région sur les octets 8 et 9 (partie de l'espace d'entropie)
        // Aligné à 100% sur le partitionnement binaire de ton AccountId
        let region_str = region.as_static_str(); // ex: "EU", "US", "ASIA"
        let region_bytes = region_str.as_bytes();

        bytes[8] = region_bytes[0];
        bytes[9] = region_bytes[1];

        Self(Uuid::from_bytes(bytes))
    }

    /// Crée une instance à partir d'une chaîne de caractères avec validation.
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }

    /// Extrait le RegionCode depuis les bits de l'UUID.
    /// Panique uniquement si l'ID est structurellement corrompu (impossible si généré via le domaine).
    pub fn region(&self) -> Region {
        let bytes = self.0.into_bytes();

        // Extraction et conversion sécurisée des octets 8 et 9
        let region_str = std::str::from_utf8(&bytes[8..10])
            .expect("ProfileId bytes 8..10 are corrupted or not valid UTF-8");

        Region::try_new(region_str)
            .expect("ProfileId contains an unmapped or invalid RegionCode")
    }

    /// Helper pour obtenir directement la chaîne de caractères statique de la région.
    /// Idéal pour alimenter la méthode `region(&self)` de ton infrastructure d'événements.
    pub fn region_str(&self) -> &'static str {
        self.region().as_static_str()
    }
}

// Implémentation du contrat d'identité générique du Shared Kernel
impl Identifier for ProfileId {
    fn as_uuid(&self) -> Uuid {
        self.0
    }

    fn as_string(&self) -> String {
        self.0.to_string()
    }

    fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl ValueObject for ProfileId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("profile_id", "Profile ID cannot be nil"));
        }

        // Barrière de sécurité aux frontières : on valide que la région codée existe
        let bytes = self.0.into_bytes();
        let region_str = std::str::from_utf8(&bytes[8..10])
            .map_err(|_| Error::validation("profile_id", "Invalid UTF-8 in region bytes"))?;

        Region::try_new(region_str).map_err(|e| {
            Error::validation("profile_id", format!("Invalid region code in ID: {}", e))
        })?;

        Ok(())
    }
}

impl Default for ProfileId {
    /// Par défaut, génère un profil sur la région historique de référence
    fn default() -> Self {
        Self::generate(Region::default())
    }
}

// --- CONVERSIONS ---

impl From<Uuid> for ProfileId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl TryFrom<String> for ProfileId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl FromStr for ProfileId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s).map(Self).map_err(|_| {
            Error::validation(
                "profile_id",
                format!("'{}' is not a valid UUID for ProfileId", s),
            )
        })
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
