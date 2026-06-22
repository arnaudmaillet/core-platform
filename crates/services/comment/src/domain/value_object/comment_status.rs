use crate::error::CommentError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStatus {
    Published = 0,
    Deleted   = 1,
}

impl CommentStatus {
    pub fn as_tinyint(self) -> i8 {
        match self {
            Self::Published => 0,
            Self::Deleted   => 1,
        }
    }
}

impl TryFrom<i8> for CommentStatus {
    type Error = CommentError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Published),
            1 => Ok(Self::Deleted),
            n => Err(CommentError::DomainViolation {
                field:   "status".into(),
                message: format!("unknown CommentStatus tinyint: {n}"),
            }),
        }
    }
}
