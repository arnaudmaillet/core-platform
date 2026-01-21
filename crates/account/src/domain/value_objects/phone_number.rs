use std::hash::{Hasher, Hash};
use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use regex::Regex;
use seahash::SeaHasher;
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

// Regex E.164 : un '+' suivi de 7 à 15 chiffres (pas de 0 après le +)
static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\+[1-9]\d{6,14}$").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PhoneNumber {
    inner: String,
    #[serde(skip)]
    hash: u64,
}

impl PhoneNumber {
    /// Constructeur sécurisé (API / Inscription)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Nettoyage agressif (on ne garde que + et chiffres)
        let cleaned: String = raw.chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect();

        let phone = Self::from_raw(cleaned);

        // 2. Validation stricte
        phone.validate()?;

        Ok(phone)
    }

    /// Reconstruction ultra-rapide (Infrastructure / DB)
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

    /// Aide au routage SMS (Hyperscale : choix du provider par région)
    pub fn country_code(&self) -> &str {
        // Logique simplifiée E.164 (les 1 à 3 premiers chiffres après le +)
        if self.inner.len() >= 4 { &self.inner[0..4] } else { &self.inner }
    }
}

impl ValueObject for PhoneNumber {
    fn validate(&self) -> Result<()> {
        // 1. Format Regex E.164
        if !PHONE_REGEX.is_match(&self.inner) {
            return Err(DomainError::Validation {
                field: "phone_number",
                reason: "Must be in E.164 format (e.g., +33612345678)".into(),
            });
        }

        // 2. Sécurité supplémentaire : interdire les suites de chiffres absurdes
        // (Optionnel selon le niveau de sévérité souhaité)
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for PhoneNumber {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<PhoneNumber> for String {
    fn from(phone: PhoneNumber) -> Self {
        phone.inner
    }
}

impl std::fmt::Display for PhoneNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}