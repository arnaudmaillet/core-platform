// crates/shared-kernel/src/building_blocks/types/phone.rs

use crate::core::{Error, Result, ValueObject};
use phf;
use regex::Regex;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/codegen_country_codes.rs"));

// Regex E.164 : un '+' suivi de 7 à 15 chiffres (pas de 0 après le +)
static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\+[1-9]\d{6,14}$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Phone {
    inner: String,
    #[serde(skip)]
    hash: u64,
}

impl Phone {
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Nettoyage agressif (on ne garde que + et chiffres)
        let cleaned: String = raw
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect();

        let phone = Self::from_raw(cleaned);

        // 2. Validation stricte
        phone.validate()?;

        Ok(phone)
    }

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

    pub fn country_code(&self) -> &str {
        let s = &self.inner;
        if s.len() < 2 {
            return "";
        }

        if s.len() >= 4 && CODES_3_DIGITS.contains(&s[1..4]) {
            return &s[1..4];
        }
        if s.len() >= 3 && CODES_2_DIGITS.contains(&s[1..3]) {
            return &s[1..3];
        }
        if s.len() >= 2 && (&s[1..2] == "1" || &s[1..2] == "7") {
            return &s[1..2];
        }
        ""
    }
}

impl ValueObject for Phone {
    fn validate(&self) -> Result<()> {
        // 1. Format Regex E.164
        if !PHONE_REGEX.is_match(&self.inner) {
            return Err(Error::validation(
                "phone_number",
                "Must be in E.164 format (e.g., +33612345678)",
            ));
        }

        // 2. Sécurité supplémentaire : interdire les suites de chiffres absurdes
        // (Optionnel selon le niveau de sévérité souhaité)
        Ok(())
    }
}

impl TryFrom<String> for Phone {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<Phone> for String {
    fn from(phone: Phone) -> Self {
        phone.inner
    }
}

impl std::fmt::Display for Phone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}
