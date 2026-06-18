// crates/post/src/domain/types/visibility_level.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityLevel {
    Public,      // Visible par tout le monde (FYP / Feed global)
    Friends,     // Followers mutuels (Amis)
    Subscribers, // Abonnés  (Contenu exclusif / Monétisation)
    Private,     // Uniquement le créateur (Brouillon sécurisé ou archive)
}

impl VisibilityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Friends => "friends",
            Self::Subscribers => "subscribers",
            Self::Private => "private",
        }
    }

    /// Helper métier : Le contenu génère-t-il des revenus directs par l'audience ?
    pub fn is_monetized(&self) -> bool {
        matches!(self, Self::Subscribers)
    }

    /// Helper pour le moteur d'indexation / algorithme (FYP)
    /// Seuls les posts publics doivent être poussés globalement.
    pub fn is_discoverable(&self) -> bool {
        matches!(self, Self::Public)
    }
}

impl ValueObject for VisibilityLevel {
    fn validate(&self) -> Result<()> {
        // L'enum garantit par construction que la valeur est valide au sens du type.
        // Si des règles de restrictions géopolitiques ou d'âge s'appliquent plus tard,
        // elles seront injectées ici.
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for VisibilityLevel {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl From<VisibilityLevel> for String {
    fn from(level: VisibilityLevel) -> Self {
        level.as_str().to_string()
    }
}

impl FromStr for VisibilityLevel {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().trim() {
            "public" => Ok(Self::Public),
            "friends" => Ok(Self::Friends),
            "subscribers" => Ok(Self::Subscribers),
            "private" => Ok(Self::Private),
            _ => Err(Error::validation(
                "visibility_level",
                format!(
                    "Unknown visibility level: '{}'. Allowed: public, friends, subscribers, private",
                    s
                ),
            )),
        }
    }
}

impl std::fmt::Display for VisibilityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
