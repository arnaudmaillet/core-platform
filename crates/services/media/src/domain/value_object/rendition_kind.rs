use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// A derivative variant in an asset's catalog. v1 variants are image size buckets;
/// the concrete output format (WebP/AVIF/JPEG) is carried by each rendition's MIME
/// type, not by this enum. `Original` is the validated master that every other
/// rendition is derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenditionKind {
    Original,
    Thumbnail,
    Small,
    Medium,
    Large,
}

impl RenditionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Thumbnail => "thumbnail",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    /// The slug used in the content-addressed key (`{kind}/{hash}/{slug}.{ext}`).
    pub fn slug(&self) -> &'static str {
        self.as_str()
    }

    /// The master is the source of truth a READY asset must always carry; the
    /// resized buckets are derived from it.
    pub fn is_original(&self) -> bool {
        matches!(self, Self::Original)
    }
}

impl fmt::Display for RenditionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for RenditionKind {
    type Error = MediaError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "original" => Ok(Self::Original),
            "thumbnail" => Ok(Self::Thumbnail),
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            other => Err(MediaError::DomainViolation {
                field: "rendition_kind".into(),
                message: format!("unknown rendition kind: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_round_trip() {
        for k in [
            RenditionKind::Original,
            RenditionKind::Thumbnail,
            RenditionKind::Small,
            RenditionKind::Medium,
            RenditionKind::Large,
        ] {
            assert_eq!(RenditionKind::try_from(k.as_str()).unwrap(), k);
        }
        assert!(RenditionKind::try_from("huge").is_err());
    }

    #[test]
    fn only_original_is_the_master() {
        assert!(RenditionKind::Original.is_original());
        assert!(!RenditionKind::Thumbnail.is_original());
    }
}
