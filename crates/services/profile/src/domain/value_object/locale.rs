use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

/// BCP-47 locale tag (e.g. `"en"`, `"en-US"`, `"zh-Hans"`).
///
/// Accepted forms:
/// - Simple language tag: 2–3 ASCII alpha characters (e.g. `"en"`, `"zho"`)
/// - Language + region: `"<lang>-<region>"` where region is 2 alpha or 3 digits
///   (e.g. `"en-US"`, `"zh-419"`)
///
/// Full BCP-47 with scripts, variants, and extensions is deliberately out of
/// scope — the profile service only needs locale for content-language hints.
static LOCALE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z]{2,3}(-([a-zA-Z]{2}|[0-9]{3}))?$").unwrap()
});

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Locale(String);

impl Locale {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        let s = raw.trim();
        if s.len() > 35 {
            return Err(ProfileError::InvalidLocale(format!(
                "locale tag too long (max 35, got {})",
                s.len()
            )));
        }
        if !LOCALE_RE.is_match(s) {
            return Err(ProfileError::InvalidLocale(format!(
                "'{s}' is not a valid BCP-47 locale tag"
            )));
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self("en".to_owned())
    }
}

impl fmt::Debug for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Locale({})", self.0)
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
