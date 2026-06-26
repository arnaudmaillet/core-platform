use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// A compact [BlurHash](https://blurha.sh) placeholder string.
///
/// It is the tiny representation a client renders *while* an asset is still
/// processing — the heart of the "publish never waits on media" guarantee: a post
/// can show a faithful blurred placeholder immediately and swap in the real
/// rendition once `AssetReady` arrives. The domain stores it opaquely; it only
/// guards against an empty or absurdly long value (a real BlurHash is on the order
/// of 20–40 chars).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Blurhash(String);

impl Blurhash {
    /// Generous upper bound for a 9×9-component BlurHash.
    pub const MAX_LEN: usize = 128;

    pub fn new(value: impl Into<String>) -> Result<Self, MediaError> {
        let raw = value.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > Self::MAX_LEN {
            return Err(MediaError::DomainViolation {
                field: "blurhash".into(),
                message: "blurhash must be non-empty and reasonably short".into(),
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Blurhash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blurhash({})", self.0)
    }
}

impl fmt::Display for Blurhash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_typical_blurhash() {
        let b = Blurhash::new("LEHV6nWB2yk8pyo0adR*.7kCMdnj").unwrap();
        assert_eq!(b.as_str(), "LEHV6nWB2yk8pyo0adR*.7kCMdnj");
    }

    #[test]
    fn rejects_empty_and_overlong() {
        assert!(Blurhash::new("   ").is_err());
        assert!(Blurhash::new("x".repeat(Blurhash::MAX_LEN + 1)).is_err());
    }
}
