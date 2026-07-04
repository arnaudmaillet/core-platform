use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// What kind of media an asset holds. `kind` is the policy discriminant: it picks
/// the MIME allowlist, the size ceiling, the rendition ladder, and the leading
/// segment of the content-addressed storage path.
///
/// v1 is **images-first**. Video is a planned fast-follow and will be added as a
/// new variant (`Video`) — additively, never by re-numbering — at which point the
/// allowlist/ceiling/ladder methods grow a video arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// A profile/account avatar — square-cropped ladder.
    Avatar,
    /// An image attached to a post — feed-sized ladder.
    PostImage,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Avatar => "avatar",
            Self::PostImage => "post_image",
        }
    }

    /// Leading segment of the content-addressed storage key (`{segment}/{hash}/…`).
    pub fn path_segment(&self) -> &'static str {
        match self {
            Self::Avatar => "avatars",
            Self::PostImage => "post-images",
        }
    }

    /// The MIME types accepted for this kind (canonical, lowercase). v1 is the
    /// modern still-image set; the concrete rendition output format is chosen by
    /// the pipeline, independent of what was uploaded.
    pub fn allowed_mime_types(&self) -> &'static [&'static str] {
        // Same still-image allowlist for both image kinds today.
        &["image/jpeg", "image/png", "image/webp", "image/avif"]
    }

    /// Hard byte ceiling enforced at ticket time and re-checked on finalize.
    pub fn max_bytes(&self) -> u64 {
        match self {
            Self::Avatar => 10 * 1024 * 1024,     // 10 MiB
            Self::PostImage => 25 * 1024 * 1024,  // 25 MiB
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for MediaKind {
    type Error = MediaError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "avatar" => Ok(Self::Avatar),
            "post_image" => Ok(Self::PostImage),
            other => Err(MediaError::InvalidMediaKind {
                kind: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_round_trip() {
        for k in [MediaKind::Avatar, MediaKind::PostImage] {
            assert_eq!(MediaKind::try_from(k.as_str()).unwrap(), k);
        }
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(matches!(
            MediaKind::try_from("hologram").unwrap_err(),
            MediaError::InvalidMediaKind { .. }
        ));
    }

    #[test]
    fn post_images_allow_a_larger_ceiling_than_avatars() {
        assert!(MediaKind::PostImage.max_bytes() > MediaKind::Avatar.max_bytes());
    }

    #[test]
    fn allowlist_is_the_modern_still_image_set() {
        assert!(MediaKind::Avatar.allowed_mime_types().contains(&"image/webp"));
        assert!(!MediaKind::Avatar.allowed_mime_types().contains(&"video/mp4"));
    }
}
