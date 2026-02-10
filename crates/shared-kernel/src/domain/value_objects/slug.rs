// crates/shared-kernel/src/domain/value_objects/slug.rs

use regex::Regex;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;
use crate::errors::{DomainError, Result};

static SLUG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._]*[a-z0-9]$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slug {
    inner: String,
    hash: u64,
}

impl Slug {
    pub const MIN_LEN: usize = 3;
    pub const MAX_LEN: usize = 30;

    pub fn try_new(value: impl Into<String>, field_name: &'static str) -> Result<Self> {
        let raw = value.into();
        // Normalisation NFC + Lowercase + Trim
        let normalized: String = raw.trim().nfc().collect::<String>().to_lowercase();

        let slug = Self::from_raw(normalized);
        slug.validate_with_name(field_name)?;
        Ok(slug)
    }

    pub fn from_raw(value: impl Into<String>) -> Self {
        let inner = value.into();
        let mut hasher = SeaHasher::new();
        inner.hash(&mut hasher);
        Self { inner, hash: hasher.finish() }
    }

    fn validate_with_name(&self, field: &'static str) -> Result<()> {
        let len = self.inner.chars().count();

        if !(Self::MIN_LEN..=Self::MAX_LEN).contains(&len) {
            return Err(DomainError::Validation {
                field: field.into(),
                reason: format!("Must be between {} and {} characters", Self::MIN_LEN, Self::MAX_LEN),
            });
        }

        if !SLUG_REGEX.is_match(&self.inner) {
            return Err(DomainError::Validation {
                field: field.into(),
                reason: "Invalid format: lowercase, numbers, dots or underscores only. Cannot start/end with special chars.".into(),
            });
        }

        if self.inner.contains("..") || self.inner.contains("__") ||
            self.inner.contains("._") || self.inner.contains("_.") {
            return Err(DomainError::Validation {
                field: field.into(),
                reason: "Cannot contain consecutive special characters".into(),
            });
        }
        Ok(())
    }

    pub fn as_str(&self) -> &str { &self.inner }
    pub fn hash_value(&self) -> u64 { self.hash }
}

impl Serialize for Slug {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_raw(s))
    }
}