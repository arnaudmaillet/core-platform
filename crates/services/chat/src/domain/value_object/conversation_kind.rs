use crate::error::ChatError;

/// Hard cap on the symmetric Member Plane of a `Group`.
///
/// This is a domain invariant, not a tunable: it bounds the cost of the
/// full-duplex presence/typing/read-receipt loops (all O(members)) and keeps the
/// member roster small enough to hydrate into the aggregate.
pub const GROUP_MAX_MEMBERS: u16 = 500;

/// Cap on the broadcaster roster of a `Channel`. A channel is `1 -> N`: only its
/// owner/admins write, while the audience subscribes read-only and is never part
/// of this roster.
pub const CHANNEL_MAX_BROADCASTERS: u16 = 200;

/// Immutable topology of a conversation.
///
/// Stored as `tinyint` in ScyllaDB and mapped to the proto enum ordinal. Per the
/// approved blueprint the topology is fixed at creation; polymorphic runtime
/// behaviour is driven by [`Visibility`](super::Visibility), not by mutating the
/// kind.
///
/// - `Group`: symmetric `N <-> N` mesh, bounded ([`GROUP_MAX_MEMBERS`]), with
///   presence, typing indicators, and individual read-receipts.
/// - `Channel`: strictly asymmetric `1 -> N` broadcast, passive reading, zero
///   presence overhead, unbounded audience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConversationKind {
    Group   = 0,
    Channel = 1,
}

impl ConversationKind {
    pub fn as_tinyint(self) -> i8 {
        self as u8 as i8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Group   => "group",
            Self::Channel => "channel",
        }
    }

    /// Whether this topology carries the high-frequency presence loops (presence,
    /// typing, read-receipts) on its Member Plane. Channels never do.
    pub fn presence_capable(self) -> bool {
        matches!(self, Self::Group)
    }

    /// Upper bound on the Member Plane roster for this topology. Always `Some`:
    /// both topologies bound their interactive roster; only the read-only
    /// Audience Plane is unbounded.
    pub fn max_members(self) -> u16 {
        match self {
            Self::Group   => GROUP_MAX_MEMBERS,
            Self::Channel => CHANNEL_MAX_BROADCASTERS,
        }
    }
}

impl TryFrom<i8> for ConversationKind {
    type Error = ChatError;

    fn try_from(v: i8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Group),
            1 => Ok(Self::Channel),
            n => Err(ChatError::UnknownConversationKind { kind: n.to_string() }),
        }
    }
}

impl TryFrom<&str> for ConversationKind {
    type Error = ChatError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "group"   => Ok(Self::Group),
            "channel" => Ok(Self::Channel),
            other     => Err(ChatError::UnknownConversationKind { kind: other.to_owned() }),
        }
    }
}
