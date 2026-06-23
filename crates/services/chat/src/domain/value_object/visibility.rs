use crate::error::ChatError;

/// Mutable visibility of a conversation — the single axis that drives the
/// polymorphic Member/Audience behaviour.
///
/// Stored as `tinyint` in ScyllaDB and mapped to the proto enum ordinal.
///
/// - `Private`: only members participate; no Audience Plane is attached, so a
///   private group pays zero broadcast/guest cost.
/// - `Public`: the Audience Plane is attached. Non-members (subscribers, guests)
///   may read history from the public-since watermark and subscribe to the
///   read-only shadow stream, while members keep interacting bidirectionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Visibility {
    Private = 0,
    Public  = 1,
}

impl Visibility {
    pub fn as_tinyint(self) -> i8 {
        self as u8 as i8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public  => "public",
        }
    }

    pub fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

impl TryFrom<i8> for Visibility {
    type Error = ChatError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Private),
            1 => Ok(Self::Public),
            n => Err(ChatError::UnknownVisibility { visibility: n.to_string() }),
        }
    }
}

impl TryFrom<&str> for Visibility {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "private" => Ok(Self::Private),
            "public"  => Ok(Self::Public),
            other     => Err(ChatError::UnknownVisibility { visibility: other.to_owned() }),
        }
    }
}
