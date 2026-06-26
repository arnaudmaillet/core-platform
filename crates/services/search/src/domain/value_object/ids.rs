use serde::{Deserialize, Serialize};

use crate::error::SearchError;

/// The responsible account for an indexed entity (a post's author, a profile's
/// own id). Held so the index can support two operations the blueprint requires:
/// caller-supplied block/mute **exclusion** at query time, and **GDPR purge**
/// (`delete_by_query` on `author_id`). It is an opaque id owned by the upstream
/// service — search never interprets it beyond equality.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorId(String);

impl AuthorId {
    /// Build a non-empty author id. An empty id is a malformed source event,
    /// surfaced as `SCH-9002 InvalidIdentifier`.
    pub fn new(value: impl Into<String>) -> Result<Self, SearchError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SearchError::InvalidIdentifier("author_id".to_owned()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn accepts_non_empty() {
        assert_eq!(AuthorId::new("acct-1").unwrap().as_str(), "acct-1");
    }

    #[test]
    fn rejects_blank() {
        let err = AuthorId::new("   ").unwrap_err();
        assert_eq!(err.error_code(), "SCH-9002");
    }
}
