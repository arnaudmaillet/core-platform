// crates/account/src/domain/value_objects/email.rs

use std::hash::{Hasher, Hash};
use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use regex::Regex;
use seahash::SeaHasher;
use unicode_normalization::UnicodeNormalization;
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

// Regex RFC 5322 simplifiée mais robuste pour le Web
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$").unwrap()
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

        // 1. Normalisation Hyperscale (NFC + Lowercase + Trim)
        let normalized: String = raw.trim()
            .nfc()
            .collect::<String>()
            .to_lowercase();

        let email = Self::new_unchecked(normalized);

        // 2. Validation
        email.validate()?;

        Ok(email)
    }

    /// Reconstruction ultra-rapide (Infrastructure / DB)
    pub fn new_unchecked(value: impl Into<String>) -> Self {
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
                reason: format!("Email length must be between 1 and {} chars", Self::MAX_LEN),
            });
        }

        if !EMAIL_REGEX.is_match(&self.address) {
            return Err(DomainError::Validation {
                field: "email",
                reason: "Invalid email format".into(),
            });
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