use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// A validated, normalized media MIME type (e.g. `image/webp`).
///
/// Construction lowercases, trims, and checks the coarse `type/subtype` shape. It
/// does NOT by itself enforce the per-kind allowlist — that is
/// [`UploadConstraints`](super::UploadConstraints)' job, since the allowlist is
/// policy and varies by [`MediaKind`](super::MediaKind). A `MimeType` only
/// guarantees the value is well-formed.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MimeType(String);

impl MimeType {
    pub fn new(value: impl Into<String>) -> Result<Self, MediaError> {
        let normalized = value.into().trim().to_ascii_lowercase();
        // Coarse RFC-ish shape check: exactly one '/', non-empty type and subtype.
        let mut parts = normalized.splitn(2, '/');
        let ok = matches!((parts.next(), parts.next()), (Some(t), Some(s)) if !t.is_empty() && !s.is_empty() && !s.contains('/'));
        if !ok {
            return Err(MediaError::UnsupportedMimeType { mime: normalized });
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_image(&self) -> bool {
        self.0.starts_with("image/")
    }

    pub fn is_video(&self) -> bool {
        self.0.starts_with("video/")
    }

    /// The conventional file extension for this type, used when building a
    /// content-addressed rendition key. Falls back to the subtype.
    pub fn extension(&self) -> &str {
        match self.0.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/avif" => "avif",
            "image/gif" => "gif",
            // Fall back to the subtype (e.g. "image/svg+xml" → "svg+xml").
            other => other.split('/').nth(1).unwrap_or("bin"),
        }
    }
}

impl fmt::Debug for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MimeType({})", self.0)
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_classifies() {
        let m = MimeType::new("  IMAGE/WebP ").unwrap();
        assert_eq!(m.as_str(), "image/webp");
        assert!(m.is_image());
        assert_eq!(m.extension(), "webp");
    }

    #[test]
    fn jpeg_extension_is_jpg() {
        assert_eq!(MimeType::new("image/jpeg").unwrap().extension(), "jpg");
    }

    #[test]
    fn rejects_malformed() {
        for bad in ["", "image", "image/", "/png", "a/b/c"] {
            assert!(
                matches!(MimeType::new(bad).unwrap_err(), MediaError::UnsupportedMimeType { .. }),
                "expected reject for {bad:?}"
            );
        }
    }
}
