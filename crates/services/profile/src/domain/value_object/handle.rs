use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ProfileError;

/// Validated, lowercased public @handle for a profile.
///
/// Rules enforced on construction:
/// - 2–30 characters after lowercasing
/// - Charset: `[a-z0-9_.]` only
/// - Must not start or end with `.` or `_`
/// - No consecutive `.` (`..`)
/// - No consecutive `_` (`__`)
/// - No `._` or `_.` sequences
static HANDLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]([a-z0-9._]*[a-z0-9])?$").unwrap());

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Handle(String);

impl Handle {
    pub fn new(raw: &str) -> Result<Self, ProfileError> {
        let lower = raw.to_ascii_lowercase();
        let s = lower.trim();

        let invalid = |msg: &str| ProfileError::InvalidHandle(format!("{raw}: {msg}"));

        if s.len() < 2 || s.len() > 30 {
            return Err(invalid("must be 2–30 characters"));
        }
        if !HANDLE_RE.is_match(s) {
            return Err(invalid("only a-z, 0-9, underscore, and dot are allowed; must start and end with alphanumeric"));
        }
        if s.contains("..") {
            return Err(invalid("consecutive dots are not allowed"));
        }
        if s.contains("__") {
            return Err(invalid("consecutive underscores are not allowed"));
        }
        if s.contains("._") || s.contains("_.") {
            return Err(invalid("dot-underscore and underscore-dot sequences are not allowed"));
        }

        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({})", self.0)
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
