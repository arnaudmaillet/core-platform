// crates/content_comments/src/domain/types/comment_id.rs
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Identifier, Result, ValueObject};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentId(Uuid);

impl CommentId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn try_new(id: impl Into<String>) -> Result<Self> {
        Self::from_str(&id.into())
    }

    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Identifier for CommentId {
    fn as_uuid(&self) -> Uuid {
        self.0
    }

    fn as_string(&self) -> String {
        self.0.to_string()
    }

    fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    fn identifier_scope() -> &'static str {
        "comment"
    }
}

impl ValueObject for CommentId {
    fn validate(&self) -> Result<()> {
        if self.0.is_nil() {
            return Err(Error::validation("comment_id", "Comment ID cannot be nil"));
        }
        Ok(())
    }
}

impl Default for CommentId {
    fn default() -> Self {
        Self::generate()
    }
}

impl From<Uuid> for CommentId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl FromStr for CommentId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s).map(Self).map_err(|_| {
            Error::validation(
                "comment_id",
                format!("'{}' is not a valid UUID for CommentId", s),
            )
        })
    }
}

impl fmt::Display for CommentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CommentId {
    type Error = shared_kernel::core::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let uuid = uuid::Uuid::parse_str(&value).map_err(|_| {
            shared_kernel::core::Error::validation("comment_id", "Invalid UUID format")
        })?;
        Ok(Self::from(uuid))
    }
}

impl TryFrom<&str> for CommentId {
    type Error = shared_kernel::core::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let uuid = uuid::Uuid::parse_str(value).map_err(|_| {
            shared_kernel::core::Error::validation("comment_id", "Invalid UUID format")
        })?;
        Ok(Self::from(uuid))
    }
}
