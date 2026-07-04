use crate::error::NotificationError;

/// Semantic type of the action that generated the notification.
///
/// The integer representation is stored as `tinyint` in ScyllaDB and matches
/// the proto enum ordinal (proto value = domain tinyint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    Reaction,
    Comment,
    Reply,
    Mention,
}

impl NotificationKind {
    pub fn as_tinyint(self) -> i8 {
        match self {
            Self::Reaction => 1,
            Self::Comment  => 2,
            Self::Reply    => 3,
            Self::Mention  => 4,
        }
    }

    pub fn from_tinyint(v: i8) -> Result<Self, NotificationError> {
        match v {
            1 => Ok(Self::Reaction),
            2 => Ok(Self::Comment),
            3 => Ok(Self::Reply),
            4 => Ok(Self::Mention),
            n => Err(NotificationError::UnknownNotificationKind { kind: n.to_string() }),
        }
    }

    pub fn from_proto(v: i32) -> Result<Self, NotificationError> {
        Self::from_tinyint(v as i8)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reaction => "reaction",
            Self::Comment  => "comment",
            Self::Reply    => "reply",
            Self::Mention  => "mention",
        }
    }
}
