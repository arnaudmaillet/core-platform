use crate::error::NotificationError;

/// Discriminates the entity acted upon — drives deep-link routing on the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubjectKind {
    Post,
    Comment,
}

impl SubjectKind {
    pub fn as_tinyint(self) -> i8 {
        match self {
            Self::Post    => 1,
            Self::Comment => 2,
        }
    }

    pub fn from_tinyint(v: i8) -> Result<Self, NotificationError> {
        match v {
            1 => Ok(Self::Post),
            2 => Ok(Self::Comment),
            n => Err(NotificationError::UnknownSubjectKind { kind: n.to_string() }),
        }
    }

    pub fn from_proto(v: i32) -> Result<Self, NotificationError> {
        Self::from_tinyint(v as i8)
    }
}
