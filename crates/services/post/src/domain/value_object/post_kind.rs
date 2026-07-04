use std::fmt;
use crate::error::PostError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostKind {
    TextOnly  = 0,
    Carousel  = 1,
    MainVideo = 2,
}

impl PostKind {
    pub fn as_tinyint(self) -> i8 {
        self as i8
    }
}

impl TryFrom<i8> for PostKind {
    type Error = PostError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::TextOnly),
            1 => Ok(Self::Carousel),
            2 => Ok(Self::MainVideo),
            _ => Err(PostError::DomainViolation {
                field:   "kind".into(),
                message: format!("unknown PostKind discriminant: {v}"),
            }),
        }
    }
}

impl fmt::Display for PostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextOnly  => write!(f, "TextOnly"),
            Self::Carousel  => write!(f, "Carousel"),
            Self::MainVideo => write!(f, "MainVideo"),
        }
    }
}
