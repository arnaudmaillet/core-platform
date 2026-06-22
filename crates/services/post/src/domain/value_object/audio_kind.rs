use std::fmt;
use crate::error::PostError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    OriginalSound = 0,
    Reused        = 1,
}

impl AudioKind {
    pub fn as_tinyint(self) -> i8 {
        self as i8
    }
}

impl TryFrom<i8> for AudioKind {
    type Error = PostError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::OriginalSound),
            1 => Ok(Self::Reused),
            _ => Err(PostError::DomainViolation {
                field:   "audio_kind".into(),
                message: format!("unknown AudioKind discriminant: {v}"),
            }),
        }
    }
}

impl fmt::Display for AudioKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OriginalSound => write!(f, "OriginalSound"),
            Self::Reused        => write!(f, "Reused"),
        }
    }
}
