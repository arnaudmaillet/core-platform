// crates/post/src/domain/types/caption.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::{collections::BTreeSet, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Caption(String);

impl Caption {
    pub const MAX_LENGTH: usize = 2200;

    pub fn try_new(text: String) -> Result<Self> {
        let cleaned = text.trim().to_string();
        let caption = Self(cleaned);
        caption.validate()?;
        Ok(caption)
    }

    pub fn from_raw(text: String) -> Self {
        Self(text)
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn extract_hashtags(&self) -> BTreeSet<String> {
        self.0
            .split_whitespace()
            .filter(|word| word.starts_with('#') && word.len() > 1)
            .map(|word| {
                word.trim_start_matches('#')
                    .trim_end_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|tag| !tag.is_empty())
            .collect()
    }

    pub fn extract_mentions(&self) -> BTreeSet<String> {
        self.0
            .split_whitespace()
            .filter(|word| word.starts_with('@') && word.len() > 1)
            .map(|word| {
                word.trim_start_matches('@')
                    .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
                    .to_string()
            })
            .filter(|mention| !mention.is_empty())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl ValueObject for Caption {
    fn validate(&self) -> Result<()> {
        if self.0.chars().count() > Self::MAX_LENGTH {
            return Err(Error::validation(
                "caption",
                format!("Caption cannot exceed {} characters", Self::MAX_LENGTH),
            ));
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for Caption {
    type Error = Error;
    fn try_from(text: String) -> Result<Self> {
        Self::try_new(text)
    }
}

impl TryFrom<&str> for Caption {
    type Error = Error;
    fn try_from(text: &str) -> Result<Self> {
        Self::try_new(text.to_string())
    }
}

impl From<Caption> for String {
    fn from(caption: Caption) -> Self {
        caption.0
    }
}

impl FromStr for Caption {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s.to_string())
    }
}

impl std::fmt::Display for Caption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
