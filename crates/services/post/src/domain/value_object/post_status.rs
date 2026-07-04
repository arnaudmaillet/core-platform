use crate::error::PostError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostStatus {
    Draft     = 0,
    Published = 1,
    Deleted   = 2,
}

impl PostStatus {
    pub fn as_tinyint(self) -> i8 {
        self as i8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft     => "Draft",
            Self::Published => "Published",
            Self::Deleted   => "Deleted",
        }
    }
}

impl TryFrom<i8> for PostStatus {
    type Error = PostError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Draft),
            1 => Ok(Self::Published),
            2 => Ok(Self::Deleted),
            _ => Err(PostError::DomainViolation {
                field:   "status".into(),
                message: format!("unknown PostStatus discriminant: {v}"),
            }),
        }
    }
}
