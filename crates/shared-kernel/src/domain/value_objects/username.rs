use std::hash::{Hasher, Hash};
use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use regex::Regex;
use seahash::SeaHasher;
use unicode_normalization::UnicodeNormalization;

use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

// Regex optimisée compilée une seule fois
static USERNAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9][a-z0-9._]*[a-z0-9]$").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username {
    inner: String,
    #[serde(skip)]
    hash: u64,
}

impl Username {
    pub const MIN_LEN: usize = 3;
    pub const MAX_LEN: usize = 30;

    /// Constructeur sécurisé (API / Domaine)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Normalisation Hyperscale (NFC + Lowercase + Trim)
        let normalized: String = raw.trim()
            .nfc()
            .collect::<String>()
            .to_lowercase();

        let username = Self::from_raw(normalized);

        // 2. Validation
        username.validate()?;

        Ok(username)
    }

    /// Reconstruction ultra-rapide (Infrastructure / DB)
    /// Calcule le hash sans re-valider la regex ou la longueur
    pub fn from_raw(value: impl Into<String>) -> Self {
        let inner = value.into();
        let mut hasher = SeaHasher::new();
        inner.hash(&mut hasher);

        Self {
            inner,
            hash: hasher.finish(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn hash_value(&self) -> u64 {
        self.hash
    }
}

impl ValueObject for Username {
    fn validate(&self) -> Result<()> {
        let len = self.inner.chars().count();

        // 1. Longueur
        if len < Self::MIN_LEN || len > Self::MAX_LEN {
            return Err(DomainError::Validation {
                field: "username",
                reason: format!("Username must be between {} and {} characters", Self::MIN_LEN, Self::MAX_LEN),
            });
        }

        // 2. Format Regex
        if !USERNAME_REGEX.is_match(&self.inner) {
            return Err(DomainError::Validation {
                field: "username",
                reason: "Invalid format: only lowercase, numbers, dots or underscores allowed. Cannot start/end with special chars.".into(),
            });
        }

        // 3. Protection contre les séquences spéciales (Anti-obfuscation)
        if self.inner.contains("..") || self.inner.contains("__") || self.inner.contains("._") || self.inner.contains("_.") {
            return Err(DomainError::Validation {
                field: "username",
                reason: "Username cannot contain consecutive special characters".into(),
            });
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for Username {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<Username> for String {
    fn from(username: Username) -> Self {
        username.inner
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}