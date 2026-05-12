use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{
    core::{Error, Result},
    domain::value_objects::ValueObject,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AuditReason {
    inner: String,
}

impl AuditReason {
    pub const MIN_LENGTH: usize = 4;
    pub const MAX_LENGTH: usize = 500;

    /// Constructeur sécurisé
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Nettoyage (Trim des espaces et gestion des retours à la ligne superflus)
        let cleaned = raw.trim().to_string();

        let reason = Self { inner: cleaned };

        // 2. Validation des invariants
        reason.validate()?;

        Ok(reason)
    }

    /// Constructeur utilitaire pour les cas systèmes internes
    pub fn system(msg: &str) -> Self {
        Self {
            inner: format!("[SYSTEM] {}", msg),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl ValueObject for AuditReason {
    fn validate(&self) -> Result<()> {
        let length = self.inner.len();

        if length < Self::MIN_LENGTH {
            return Err(Error::validation(
                "audit_reason",
                format!("Reason is too short (min {} chars)", Self::MIN_LENGTH),
            ));
        }

        if length > Self::MAX_LENGTH {
            return Err(Error::validation(
                "audit_reason",
                format!("Reason is too long (max {} chars)", Self::MAX_LENGTH),
            ));
        }

        // Optionnel : On peut ici rejeter des patterns suspects (HTML tags, etc.)
        Ok(())
    }
}

// --- CONVERSIONS & TRAITS ---

impl TryFrom<String> for AuditReason {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<AuditReason> for String {
    fn from(reason: AuditReason) -> Self {
        reason.inner
    }
}

impl fmt::Display for AuditReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}
