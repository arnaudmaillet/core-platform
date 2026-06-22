use crate::error::ChatError;

/// Payload type of a chat message.
///
/// Stored as `tinyint` in ScyllaDB and mapped to the proto enum ordinal.
///
/// - `Text`: a textual body (must be non-empty).
/// - `Media`: an image/video/audio attachment referenced by an out-of-band
///   pointer (`media_ref`); large media is never inlined into the message log.
/// - `System`: a service-generated event rendered inline (e.g. "made public",
///   "X joined"); body is service-controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentType {
    Text   = 0,
    Media  = 1,
    System = 2,
}

impl ContentType {
    pub fn as_tinyint(self) -> i8 {
        self as u8 as i8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text   => "text",
            Self::Media  => "media",
            Self::System => "system",
        }
    }
}

impl TryFrom<i8> for ContentType {
    type Error = ChatError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Text),
            1 => Ok(Self::Media),
            2 => Ok(Self::System),
            n => Err(ChatError::UnknownContentType { content_type: n.to_string() }),
        }
    }
}

impl TryFrom<&str> for ContentType {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "text"   => Ok(Self::Text),
            "media"  => Ok(Self::Media),
            "system" => Ok(Self::System),
            other    => Err(ChatError::UnknownContentType { content_type: other.to_owned() }),
        }
    }
}
