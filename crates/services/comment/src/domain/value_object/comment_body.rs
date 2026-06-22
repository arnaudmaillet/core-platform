use crate::error::CommentError;

const MAX_BODY_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentBody(String);

impl CommentBody {
    pub fn new(s: impl Into<String>) -> Result<Self, CommentError> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(CommentError::DomainViolation {
                field:   "body".into(),
                message: "comment body must not be blank".into(),
            });
        }
        let len = s.chars().count();
        if len > MAX_BODY_CHARS {
            return Err(CommentError::DomainViolation {
                field:   "body".into(),
                message: format!(
                    "comment body must be at most {MAX_BODY_CHARS} characters (got {len})"
                ),
            });
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<CommentBody> for String {
    fn from(b: CommentBody) -> Self {
        b.0
    }
}
