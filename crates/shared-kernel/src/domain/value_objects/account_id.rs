// crates/shared-kernel/src/domain/value_object/account_id.rs

use crate::domain::identifier::Identifier;
use crate::domain::value_objects::{RegionCode, ValueObject};
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique composite pour un compte.
/// Encapsule l'identité technique (UUID) et la localisation (RegionCode).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId {
    uuid: Uuid,
    region: RegionCode,
}

impl AccountId {
    /// Crée un nouvel identifiant à partir d'un UUID et d'un RegionCode.
    pub fn new(uuid: Uuid, region: RegionCode) -> Self {
        Self { uuid, region }
    }

    /// Génère un nouvel identifiant (UUID v7) pour une région donnée.
    pub fn generate(region: RegionCode) -> Self {
        Self::new(Uuid::now_v7(), region)
    }

    /// Accesseur pour l'UUID.
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Accesseur pour le RegionCode.
    pub fn region(&self) -> &RegionCode {
        &self.region
    }
}

// Implémentation du contrat d'identité
impl Identifier for AccountId {
    fn as_uuid(&self) -> Uuid {
        self.uuid
    }

    /// Retourne "REGION:UUID" (ex: "EU:018f3a...")
    fn as_string(&self) -> String {
        format!("{}:{}", self.region, self.uuid)
    }

    fn from_uuid(uuid: Uuid) -> Self {
        Self {
            uuid,
            region: RegionCode::default(),
        }
    }
}

impl ValueObject for AccountId {
    fn validate(&self) -> Result<()> {
        if self.uuid.is_nil() {
            return Err(DomainError::Validation {
                field: "account_id",
                reason: "UUID part cannot be nil".to_string(),
            });
        }
        // La validation de la région est déléguée au VO RegionCode
        self.region.validate()
    }
}

// --- CONVERSIONS ---

impl FromStr for AccountId {
    type Err = DomainError;

    /// Parse une chaîne au format "REGION:UUID"
    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() != 2 {
            return Err(DomainError::Validation {
                field: "account_id",
                reason: format!("'{}' is not a valid AccountId. Expected 'REGION:UUID'", s),
            });
        }

        // On utilise FromStr de RegionCode qui valide déjà le code
        let region = RegionCode::from_str(parts[0])?;

        let uuid = Uuid::parse_str(parts[1]).map_err(|_| DomainError::Validation {
            field: "account_id",
            reason: format!("'{}' is not a valid UUID", parts[1]),
        })?;

        Ok(Self { uuid, region })
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.region, self.uuid)
    }
}

impl TryFrom<String> for AccountId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for AccountId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_str(value)
    }
}
