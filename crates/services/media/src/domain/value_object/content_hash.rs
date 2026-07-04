use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// A lowercase-hex **SHA-256** digest of an asset's original bytes (64 hex chars).
///
/// This is the load-bearing value for two of the service's guarantees:
/// * **content-addressing** — it forms the immutable segment of every rendition's
///   [`StorageKey`](super::StorageKey), so identical bytes always map to the same
///   key and an edit (different bytes) lands on a different URL; and
/// * **dedup** — when enabled, two uploads with the same hash share stored bytes.
///
/// Construction normalizes to lowercase and rejects anything that is not exactly
/// 64 hex characters, so a malformed digest is unrepresentable.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    /// Validates and normalizes a hex SHA-256 string.
    pub fn new(value: impl Into<String>) -> Result<Self, MediaError> {
        let raw = value.into();
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.len() != 64 || !normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(MediaError::DomainViolation {
                field: "content_hash".into(),
                message: "expected a 64-character hex SHA-256 digest".into(),
            });
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.0)
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn accepts_and_lowercases_a_valid_digest() {
        let h = ContentHash::new(VALID.to_ascii_uppercase()).unwrap();
        assert_eq!(h.as_str(), VALID);
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert!(ContentHash::new("abc123").is_err());
        assert!(ContentHash::new("z".repeat(64)).is_err());
        assert!(matches!(
            ContentHash::new("").unwrap_err(),
            MediaError::DomainViolation { .. }
        ));
    }

    #[test]
    fn same_bytes_same_hash_is_eq() {
        assert_eq!(ContentHash::new(VALID).unwrap(), ContentHash::new(VALID).unwrap());
    }
}
