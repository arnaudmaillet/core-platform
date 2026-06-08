// crates/content_comments/src/domain/types/comment_content.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

const MAX_COMMENT_LENGTH: usize = 2200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentContent(String);

impl CommentContent {
    pub fn try_new(content: impl Into<String>) -> Result<Self> {
        let content_str = content.into();
        let vo = Self(content_str.trim().to_string());
        vo.validate()?;
        Ok(vo)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for CommentContent {
    fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::validation(
                "comment_content",
                "Comment content cannot be empty",
            ));
        }

        if self.0.chars().count() > MAX_COMMENT_LENGTH {
            return Err(Error::validation(
                "comment_content",
                format!(
                    "Comment exceeds maximum length of {} characters",
                    MAX_COMMENT_LENGTH
                ),
            ));
        }

        Ok(())
    }
}

impl AsRef<str> for CommentContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CommentContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
