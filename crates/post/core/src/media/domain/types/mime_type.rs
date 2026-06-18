// crates/post/src/domain/types/mime_type.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MimeType(String);

impl MimeType {
    pub fn try_new(mime: String) -> Result<Self> {
        let cleaned = mime.trim().to_lowercase();
        let mime_type = Self(cleaned);
        mime_type.validate()?;
        Ok(mime_type)
    }

    /// Reconstruit le Value Object depuis la base sans re-déclencher les validations
    pub fn from_raw(mime: String) -> Self {
        Self(mime)
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn is_video(&self) -> bool {
        self.0.starts_with("video/")
    }

    pub fn is_image(&self) -> bool {
        self.0.starts_with("image/")
    }
}

impl ValueObject for MimeType {
    fn validate(&self) -> Result<()> {
        match self.0.as_str() {
            // Formats vidéo autorisés
            "video/mp4" | "video/quicktime" | "video/webm" => Ok(()),

            // Formats image autorisés
            "image/jpeg" | "image/png" | "image/webp" => Ok(()),

            _ => Err(Error::validation(
                "mime_type",
                format!(
                    "Unsupported MIME type: '{}'. Allowed: video/mp4, video/quicktime, video/webm, image/jpeg, image/png, image/webp",
                    self.0
                ),
            )),
        }
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for MimeType {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for MimeType {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        Self::try_new(value.to_string())
    }
}

impl From<MimeType> for String {
    fn from(mime: MimeType) -> Self {
        mime.0
    }
}

impl FromStr for MimeType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s.to_string())
    }
}

impl std::fmt::Display for MimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
