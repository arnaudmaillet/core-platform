// crates/account/src/domain/value_objects/email.rs

use regex::Regex;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

// Regex RFC 5322 simplifiée mais robuste pour le Web
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Email {
    address: String,
    #[serde(skip)]
    hash: u64,
}

impl Email {
    pub const MAX_LEN: usize = 254;

    /// Constructeur sécurisé (API / Inscription)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();
        let trimmed = raw.trim().to_lowercase();

        // On passe par idna pour la structure
        let (idna_out, _) = idna::uts46::Uts46::new().to_unicode(
            trimmed.as_bytes(),
            idna::uts46::AsciiDenyList::EMPTY,
            idna::uts46::Hyphens::Allow
        );

        // ON FORCE LE RÉSULTAT : on fusionne l'accent manuellement
        // On transforme "e" + "accent flottant" en "é" composé
        let normalized = idna_out.replace("e\u{0301}", "\u{00e9}");

        let email = Self::from_raw(normalized);
        email.validate()?;
        Ok(email)
    }

    /// Reconstruction ultra-rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        let address = value.into();
        let mut hasher = SeaHasher::new();
        address.hash(&mut hasher);

        Self {
            address,
            hash: hasher.finish(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.address
    }

    pub fn hash_value(&self) -> u64 {
        self.hash
    }

    pub fn domain(&self) -> &str {
        self.address.split('@').last().unwrap_or("")
    }
}

impl ValueObject for Email {
    fn validate(&self) -> Result<()> {
        let len = self.address.len();
        if len == 0 || len > Self::MAX_LEN {
            return Err(DomainError::Validation {
                field: "email",
                reason: format!("Email length must be between 1 and {} chars", Self::MAX_LEN)
            });
        }

        let parts: Vec<&str> = self.address.split('@').collect();
        if parts.len() != 2 {
            return Err(DomainError::Validation { field: "email", reason: "Must contain one @".into() });
        }

        let local = parts[0];
        let domain = parts[1];

        // Validation Partie Locale (Rust Natif = 100% Unicode Safe)
        if local.is_empty() || local.starts_with('.') || local.ends_with('.') || local.contains("..") {
            return Err(DomainError::Validation { field: "email", reason: "Invalid local part structure".into() });
        }

        // Validation Domaine (Regex ASCII = 100% Stable)
        if !EMAIL_REGEX.is_match(domain) {
            return Err(DomainError::Validation { field: "email", reason: "Invalid domain format".into() });
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for Email {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<Email> for String {
    fn from(email: Email) -> Self {
        email.address
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.address)
    }
}
