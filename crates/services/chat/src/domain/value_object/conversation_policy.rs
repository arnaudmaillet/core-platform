use super::{ConversationKind, Visibility};

/// Derived runtime policy for a conversation — the single source of
/// **infrastructure gating**.
///
/// Computed purely from `(kind, visibility)` via [`ConversationPolicy::derive`].
/// This is the strategy-object form of polymorphism approved in the blueprint
/// (mirroring `timeline`'s `FanOutMode`): one aggregate, no subtype hierarchy,
/// behaviour switched by data. The application/infrastructure layers read these
/// flags to decide which physical paths to instantiate, so a private group pays
/// nothing for presence-less or audience infrastructure it does not use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationPolicy {
    /// Whether the Member Plane runs presence (online/last-seen) signalling.
    pub presence_enabled: bool,

    /// Whether typing indicators are emitted (Member Plane only).
    pub typing_enabled: bool,

    /// Whether individual read-receipts are tracked (Member Plane only, O(members)).
    pub receipts_enabled: bool,

    /// Upper bound on the interactive Member Plane roster.
    pub max_members: u16,

    /// Whether the read-only Audience Plane is attached. `true` iff the
    /// conversation is public; attaching it opens the sharded broadcast channels
    /// and admits subscribers/guests.
    pub audience_plane: bool,
}

impl ConversationPolicy {
    /// Derives the policy from the immutable topology and the mutable visibility.
    ///
    /// Presence/typing/receipts follow the topology
    /// ([`ConversationKind::presence_capable`]); the Audience Plane follows the
    /// visibility ([`Visibility::is_public`]). These two axes are orthogonal,
    /// which is exactly why a public group keeps full member presence while
    /// shedding the audience onto a separate plane.
    pub fn derive(kind: ConversationKind, visibility: Visibility) -> Self {
        let presence = kind.presence_capable();
        Self {
            presence_enabled: presence,
            typing_enabled:   presence,
            receipts_enabled: presence,
            max_members:      kind.max_members(),
            audience_plane:   visibility.is_public(),
        }
    }
}
