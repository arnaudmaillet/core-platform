// crates/profile/src/domain/types/profile_id.rs
use crate::core::{Error, Identifier, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant unique "Smart-Type" pour un profil, basé sur un UUID v7 standard.
/// La localisation et le routage infrastructure sont désormais délégués au contexte applicatif.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    /// Génère un nouvel identifiant unique (UUID v7 standard).
    /// Garantit l'ordre temporel natif et la conformité stricte avec la RFC 9562.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Crée une instance à partir d'une chaîne de caractères avec validation.
    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }

    /// Extrait l'UUID brut sous-jacent (utile pour les drivers comme sqlx / scylladb).
    pub fn uuid(&self) -> Uuid {
        self.0
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

    fn identifier_scope() -> &'static str {
        "profile"
    }
}

impl ValueObject for ProfileId {
    /// Barrière de sécurité aux frontières du domaine.
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("profile_id", "Profile ID cannot be nil"));
        }

        // Note : Plus besoin de valider les octets de région ici.
        // L'UUID v7 est structurellement valide par construction via la bibliothèque `uuid`.
        Ok(())
    }
}

impl Default for ProfileId {
    /// Génère un identifiant par défaut basé sur le timestamp courant.
    fn default() -> Self {
        Self::generate()
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
