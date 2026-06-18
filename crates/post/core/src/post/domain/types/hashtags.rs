// crates/post/src/domain/types/hashtags.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Hashtags(BTreeSet<String>);

impl Hashtags {
    pub const MAX_TAGS_COUNT: usize = 20;
    pub const MAX_TAG_LENGTH: usize = 50;

    pub fn try_new(tags: BTreeSet<String>) -> Result<Self> {
        let cleaned: BTreeSet<String> = tags
            .into_iter()
            .map(|tag| tag.trim_start_matches('#').trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();

        let hashtags = Self(cleaned);
        hashtags.validate()?;

        Ok(hashtags)
    }
    pub fn from_raw(tags: BTreeSet<String>) -> Self {
        Self(tags)
    }

    pub fn value(&self) -> &BTreeSet<String> {
        &self.0
    }

    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, tag: &str) -> bool {
        self.0.contains(&tag.to_lowercase())
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, String> {
        self.0.iter()
    }
}

impl ValueObject for Hashtags {
    fn validate(&self) -> Result<()> {
        if self.0.len() > Self::MAX_TAGS_COUNT {
            return Err(Error::validation(
                "hashtags",
                format!(
                    "A post cannot have more than {} hashtags",
                    Self::MAX_TAGS_COUNT
                ),
            ));
        }

        for tag in &self.0 {
            let char_count = tag.chars().count();

            if char_count > Self::MAX_TAG_LENGTH {
                return Err(Error::validation(
                    "hashtags",
                    format!(
                        "Hashtag '{}' exceeds the maximum length of {} characters",
                        tag,
                        Self::MAX_TAG_LENGTH
                    ),
                ));
            }

            // Validation des caractères autorisés (lettres, chiffres, underscores)
            // On refuse la ponctuation ou les caractères spéciaux dans le tag lui-même
            if !tag
                .trim_start_matches('#')
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err(Error::validation(
                    "hashtags",
                    format!(
                        "Hashtag '{}' contains invalid characters. Only alphanumeric and underscores are allowed",
                        tag
                    ),
                ));
            }
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<Vec<String>> for Hashtags {
    type Error = Error;
    fn try_from(tags: Vec<String>) -> Result<Self> {
        let set: BTreeSet<String> = tags.into_iter().collect();
        Self::try_new(set)
    }
}

impl From<Hashtags> for Vec<String> {
    fn from(hashtags: Hashtags) -> Self {
        hashtags.0.into_iter().collect()
    }
}

impl From<Hashtags> for BTreeSet<String> {
    fn from(hashtags: Hashtags) -> Self {
        hashtags.0
    }
}

impl<'a> IntoIterator for &'a Hashtags {
    type Item = &'a String;
    type IntoIter = std::collections::btree_set::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
