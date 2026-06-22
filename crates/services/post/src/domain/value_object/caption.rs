use crate::error::PostError;

const MAX_CAPTION_CHARS: usize = 2200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caption(String);

impl Caption {
    pub fn new(s: impl Into<String>) -> Result<Self, PostError> {
        let s = s.into();
        let len = s.chars().count();
        if len > MAX_CAPTION_CHARS {
            return Err(PostError::DomainViolation {
                field:   "caption".into(),
                message: format!("caption must be at most {MAX_CAPTION_CHARS} characters (got {len})"),
            });
        }
        Ok(Self(s))
    }

    pub fn empty() -> Self {
        Self(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<Caption> for String {
    fn from(c: Caption) -> Self {
        c.0
    }
}
