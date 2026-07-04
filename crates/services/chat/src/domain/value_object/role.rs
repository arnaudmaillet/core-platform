use crate::error::ChatError;

/// Role of a profile relative to a conversation.
///
/// The five roles split across the **aggregate consistency boundary** — this cut
/// is the core of the Shadowing Pattern:
///
/// - `Owner`, `Admin`, `Member` are the **Member Plane**: they live *inside* the
///   aggregate (they count against the bounded roster and emit presence). They
///   are persisted as rows in `members_by_conversation`.
/// - `Subscriber`, `Guest` are the **Audience Plane**: read-only, never hydrated
///   into the aggregate, tracked separately in the subscription read model. They
///   must never be constructed as a [`Participant`](crate::domain::aggregate::Participant).
///
/// Stored as `tinyint` in ScyllaDB (member-plane roles only) and mapped to the
/// proto enum ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    Owner      = 0,
    Admin      = 1,
    Member     = 2,
    Subscriber = 3,
    Guest      = 4,
}

impl Role {
    pub fn as_tinyint(self) -> i8 {
        self as u8 as i8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner      => "owner",
            Self::Admin      => "admin",
            Self::Member     => "member",
            Self::Subscriber => "subscriber",
            Self::Guest      => "guest",
        }
    }

    /// Whether this role belongs to the bounded, full-duplex Member Plane (and is
    /// therefore inside the aggregate boundary).
    pub fn is_member_plane(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Member)
    }

    /// Whether this role may administer the conversation — in particular toggle
    /// its [`Visibility`](super::Visibility).
    pub fn can_administer(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Whether this role may post messages. All member-plane roles may write; in
    /// a `Channel` only owners/admins are member-plane, so this naturally yields
    /// broadcast-only writes there.
    pub fn can_write(self) -> bool {
        self.is_member_plane()
    }

    /// Whether this role may emit presence/typing/read-receipt signals. Bounded
    /// to the Member Plane and further gated by
    /// [`ConversationPolicy::presence_enabled`](super::ConversationPolicy).
    pub fn can_emit_presence(self) -> bool {
        self.is_member_plane()
    }

    /// Every role may read (subject to the public-since watermark for the
    /// audience). Encoded explicitly for call-site clarity.
    pub fn can_read(self) -> bool {
        true
    }
}

impl TryFrom<i8> for Role {
    type Error = ChatError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Owner),
            1 => Ok(Self::Admin),
            2 => Ok(Self::Member),
            3 => Ok(Self::Subscriber),
            4 => Ok(Self::Guest),
            n => Err(ChatError::UnknownRole { role: n.to_string() }),
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "owner"      => Ok(Self::Owner),
            "admin"      => Ok(Self::Admin),
            "member"     => Ok(Self::Member),
            "subscriber" => Ok(Self::Subscriber),
            "guest"      => Ok(Self::Guest),
            other        => Err(ChatError::UnknownRole { role: other.to_owned() }),
        }
    }
}
