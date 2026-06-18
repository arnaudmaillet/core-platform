// crates/post/src/domain/types/media_type.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Video,
    Image,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Image => "image",
        }
    }

    pub fn is_video(&self) -> bool {
        matches!(self, Self::Video)
    }

    pub fn is_image(&self) -> bool {
        matches!(self, Self::Image)
    }
}

impl ValueObject for MediaType {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for MediaType {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_str(&value)
    }
}

impl From<MediaType> for String {
    fn from(media_type: MediaType) -> Self {
        media_type.as_str().to_string()
    }
}

impl FromStr for MediaType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().trim() {
            "video" => Ok(Self::Video),
            "image" => Ok(Self::Image),
            _ => Err(Error::validation(
                "media_type",
                format!("Unknown media type: '{}'. Allowed values: video, image", s),
            )),
        }
    }
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
