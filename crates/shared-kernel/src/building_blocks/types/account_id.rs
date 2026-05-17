// crates/shared-kernel/src/building_blocks/types/account_id.rs

use crate::core::{Error, Identifier, Result, ValueObject};
use crate::types::RegionCode;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique "Smart" pour un compte.
/// Auto-porte sa région géographique directement codée dans ses 128 bits binaires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Crée un AccountId à partir d'un UUID existant (en extrait la région à la volée)
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Génère un nouvel AccountId (UUID v7) en y injectant dynamiquement la région
    pub fn generate(region: RegionCode) -> Self {
        // 1. Génération d'un UUID v7 standard (contient le timestamp actuel)
        let mut bytes = Uuid::now_v7().into_bytes();

        // 2. Injection de la région sur les octets 6 et 7 (partie de l'espace d'entropie)
        // Note: L'UUID v7 utilise les octets 0..6 pour le timestamp. L'octet 6 contient la version.
        // Nous écrivons sur les octets 8 et 9 pour être totalement safe vis-à-vis des bits de métadonnées.
        let region_str = region.as_str(); // ex: "EU", "US"
        let region_bytes = region_str.as_bytes();

        bytes[8] = region_bytes[0];
        bytes[9] = region_bytes[1];

        Self(Uuid::from_bytes(bytes))
    }

    /// Accesseur pour l'UUID brut sous-jacent (utile pour sqlx)
    pub fn uuid(&self) -> Uuid {
        self.0
    }

    /// Extrait instantanément le RegionCode depuis les bits de l'UUID
    pub fn region(&self) -> RegionCode {
        let bytes = self.0.into_bytes();

        // 1. Extraction et conversion des octets 8 et 9 en chaîne
        let region_str = std::str::from_utf8(&bytes[8..10])
            .expect("AccountId bytes 8..10 are corrupted or not valid UTF-8");

        // 2. Reconstruction du Value Object de région
        RegionCode::try_new(region_str.to_string())
            .expect("AccountId contains an unmapped or invalid RegionCode")
    }

    /// Construit un AccountId "Smart" à partir d'un UUID externe (ex: Fournisseur d'identité / OAuth)
    /// en y gravant de force les octets de la région de destination.
    pub fn from_external_uuid(external_uuid: Uuid, region: RegionCode) -> Self {
        let mut bytes = external_uuid.into_bytes();
        let region_bytes = region.as_static_str().as_bytes();

        // Gravure des octets de la région au même endroit (octets 8 et 9)
        bytes[8] = region_bytes[0];
        bytes[9] = region_bytes[1];

        Self(Uuid::from_bytes(bytes))
    }
}

// Implémentation du contrat d'identité universel
impl Identifier for AccountId {
    fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Retourne la représentation textuelle canonique de l'UUID (plus besoin du préfixe "EU:")
    fn as_string(&self) -> String {
        self.0.to_string()
    }

    /// Plus aucune triche ici ! La méthode est capable de reconstruire l'ID complet avec sa région
    fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl ValueObject for AccountId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("account_id", "UUID cannot be nil"));
        }

        // Ici, on teste si l'extraction fonctionne sans faire crasher l'application
        let bytes = self.0.into_bytes();
        let region_str = std::str::from_utf8(&bytes[8..10])
            .map_err(|_| Error::validation("account_id", "Invalid UTF-8 in region bytes"))?;

        RegionCode::try_new(region_str.to_string())
            .map_err(|e| Error::validation("account_id", format!("Invalid region code: {}", e)))?;

        Ok(())
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountId {
    type Err = Error;

    /// Supporte le parsing d'un UUID standard textuel
    fn from_str(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| {
            Error::validation("account_id", format!("'{}' is not a valid UUID string", s))
        })?;
        Ok(Self(uuid))
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for AccountId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for AccountId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}
