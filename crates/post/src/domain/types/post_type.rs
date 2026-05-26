// crates/post/src/domain/types/post_type.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostType {
    Video,    // Une vidéo unique en plein écran (Format standard TikTok)
    Carousel, // Un diaporama de plusieurs images/vidéos (Format swipe)
    Image,    // Une image unique statique
    Text,     // Un post textuel pur (sans aucun média associé)
}

impl PostType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Carousel => "carousel",
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    pub fn requires_media(&self) -> bool {
        !matches!(self, Self::Text)
    }

    pub fn is_carousel(&self) -> bool {
        matches!(self, Self::Carousel)
    }
}

impl ValueObject for PostType {
    fn validate(&self) -> Result<()> {
        // L'invariant fort est garanti par la structure stricte de l'enum Rust.
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for PostType {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl From<PostType> for String {
    fn from(post_type: PostType) -> Self {
        post_type.as_str().to_string()
    }
}

impl FromStr for PostType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().trim() {
            "video" => Ok(Self::Video),
            "carousel" => Ok(Self::Carousel),
            "image" => Ok(Self::Image),
            "text" => Ok(Self::Text),
            _ => Err(Error::validation(
                "post_type",
                format!(
                    "Unknown post type: '{}'. Allowed: video, carousel, image, text",
                    s
                ),
            )),
        }
    }
}

impl std::fmt::Display for PostType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
