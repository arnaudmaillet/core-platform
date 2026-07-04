use serde::{Deserialize, Serialize};

use crate::domain::value_object::{MediaKind, MimeType};
use crate::error::MediaError;

/// The policy a [`MediaKind`] imposes on an upload: the MIME allowlist and the
/// byte ceiling. It is the single place that turns "what the client declared" into
/// an accept/reject, and it is reused on finalize to re-check the *verified* MIME
/// and *actual* size against the same rules.
///
/// Defaults are derived from the kind ([`UploadConstraints::for_kind`]); the
/// application may inject overrides in a later phase, but the domain ships sane,
/// testable defaults so the rules are enforced from day one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadConstraints {
    allowed_mime_types: Vec<String>,
    max_bytes: u64,
}

impl UploadConstraints {
    /// The default constraints for a kind (its allowlist + ceiling).
    pub fn for_kind(kind: MediaKind) -> Self {
        Self {
            allowed_mime_types: kind
                .allowed_mime_types()
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            max_bytes: kind.max_bytes(),
        }
    }

    /// Construct explicit constraints (application override).
    pub fn new(allowed_mime_types: Vec<String>, max_bytes: u64) -> Self {
        Self {
            allowed_mime_types,
            max_bytes,
        }
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    pub fn allows(&self, mime: &MimeType) -> bool {
        self.allowed_mime_types.iter().any(|m| m == mime.as_str())
    }

    /// Validates a (mime, size) pair against the policy. Returns the specific,
    /// caller-actionable error: an off-allowlist type is `UnsupportedMimeType`
    /// (MED-1002); an oversize upload is `UploadSizeExceeded` (MED-1003).
    pub fn validate(&self, mime: &MimeType, size_bytes: u64) -> Result<(), MediaError> {
        if !self.allows(mime) {
            return Err(MediaError::UnsupportedMimeType {
                mime: mime.as_str().to_owned(),
            });
        }
        if size_bytes > self.max_bytes {
            return Err(MediaError::UploadSizeExceeded {
                limit: self.max_bytes,
                actual: size_bytes,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn webp() -> MimeType {
        MimeType::new("image/webp").unwrap()
    }

    #[test]
    fn accepts_an_allowed_type_within_the_ceiling() {
        let c = UploadConstraints::for_kind(MediaKind::PostImage);
        assert!(c.validate(&webp(), 1_000_000).is_ok());
    }

    #[test]
    fn rejects_an_off_allowlist_type() {
        let c = UploadConstraints::for_kind(MediaKind::Avatar);
        let mp4 = MimeType::new("video/mp4").unwrap();
        assert!(matches!(
            c.validate(&mp4, 10).unwrap_err(),
            MediaError::UnsupportedMimeType { .. }
        ));
    }

    #[test]
    fn rejects_an_oversize_upload_with_the_limit() {
        let c = UploadConstraints::for_kind(MediaKind::Avatar);
        let over = MediaKind::Avatar.max_bytes() + 1;
        assert!(matches!(
            c.validate(&webp(), over).unwrap_err(),
            MediaError::UploadSizeExceeded { limit, actual }
                if limit == MediaKind::Avatar.max_bytes() && actual == over
        ));
    }
}
