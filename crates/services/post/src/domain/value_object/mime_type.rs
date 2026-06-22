use serde::{Deserialize, Serialize};
use crate::error::PostError;

const ALLOWED: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "image/heic",
    "video/mp4",
    "video/quicktime",
    "video/webm",
    "video/x-msvideo",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MimeType(String);

impl MimeType {
    pub fn new(s: impl Into<String>) -> Result<Self, PostError> {
        let s = s.into();
        if !ALLOWED.contains(&s.as_str()) {
            return Err(PostError::InvalidMimeType { mime_type: s });
        }
        Ok(Self(s))
    }

    pub fn is_video(&self) -> bool {
        self.0.starts_with("video/")
    }

    pub fn is_image(&self) -> bool {
        self.0.starts_with("image/")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<MimeType> for String {
    fn from(m: MimeType) -> Self {
        m.0
    }
}
