use serde::{Deserialize, Serialize};

use crate::error::RealtimeError;

/// The delivery guarantee a channel class carries. Drives whether a delivered
/// event sets `ack_required` (and is tracked against an ack watermark) or is
/// pure fire-and-forget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// The client must acknowledge; the server tracks an ack watermark. Used for
    /// DM / NOTIFICATION, where a missed item matters (and is re-synced from the
    /// owning SoR on reconnect).
    AtLeastOnce,
    /// No ack; a dropped frame is superseded by the next. Used for PRESENCE /
    /// COUNTER / FEED, where only the latest state matters.
    FireAndForget,
}

/// The multiplex channel class. Mirrors `realtime.v1.ChannelClass` but is a pure
/// domain type — the proto mapping (and its `+1`/`UNSPECIFIED` offset) lives in
/// the infrastructure tier, keeping the domain free of generated types.
///
/// The split between *identity-scoped* (DM / NOTIFICATION / PRESENCE) and
/// *public* (COUNTER / FEED) classes is load-bearing: it is the entire basis of
/// the plane's authorization (see [`super::super::session::Session::authorize`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelClass {
    Dm,
    Notification,
    Presence,
    Counter,
    Feed,
}

impl ChannelClass {
    /// Stable lowercase discriminant used in channel rendering, registry keys,
    /// and event mapping. Part of the wire/storage contract — treat changes as a
    /// migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelClass::Dm => "dm",
            ChannelClass::Notification => "notif",
            ChannelClass::Presence => "presence",
            ChannelClass::Counter => "counter",
            ChannelClass::Feed => "feed",
        }
    }

    /// Parse the stable discriminant. An unrecognized value is a contract fault,
    /// surfaced as `RTM-9001 DomainViolation`.
    pub fn try_from_str(s: &str) -> Result<Self, RealtimeError> {
        match s {
            "dm" => Ok(ChannelClass::Dm),
            "notif" => Ok(ChannelClass::Notification),
            "presence" => Ok(ChannelClass::Presence),
            "counter" => Ok(ChannelClass::Counter),
            "feed" => Ok(ChannelClass::Feed),
            other => Err(RealtimeError::DomainViolation {
                field: "channel_class".to_owned(),
                message: format!("unknown channel class '{other}'"),
            }),
        }
    }

    /// Whether the channel key must equal the connection's pinned identity. True
    /// for the per-user classes (DM / NOTIFICATION / PRESENCE); false for the
    /// public entity classes (COUNTER / FEED).
    pub fn is_identity_scoped(&self) -> bool {
        matches!(
            self,
            ChannelClass::Dm | ChannelClass::Notification | ChannelClass::Presence
        )
    }

    /// The delivery guarantee for this class.
    pub fn delivery(&self) -> DeliveryGuarantee {
        match self {
            ChannelClass::Dm | ChannelClass::Notification => DeliveryGuarantee::AtLeastOnce,
            ChannelClass::Presence | ChannelClass::Counter | ChannelClass::Feed => {
                DeliveryGuarantee::FireAndForget
            }
        }
    }

    /// Convenience: whether a delivered event on this class requires a client ack.
    pub fn ack_required(&self) -> bool {
        self.delivery() == DeliveryGuarantee::AtLeastOnce
    }
}

/// The qualifier a channel class scopes to: the user id for identity-scoped
/// classes, the entity id for public ones. Opaque, non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelKey(String);

impl ChannelKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RealtimeError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RealtimeError::InvalidIdentifier("channel_key".to_owned()));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A channel is a `(class, key)` pair — the addressable unit a connection
/// subscribes to and an event is delivered on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelRef {
    pub class: ChannelClass,
    pub key: ChannelKey,
}

impl ChannelRef {
    pub fn new(class: ChannelClass, key: ChannelKey) -> Self {
        Self { class, key }
    }
}

impl std::fmt::Display for ChannelRef {
    /// Renders as `class:key` (e.g. `dm:alice`, `counter:post-42`) — the form used
    /// in error messages and logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.class.as_str(), self.key.as_str())
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn class_round_trips() {
        for class in [
            ChannelClass::Dm,
            ChannelClass::Notification,
            ChannelClass::Presence,
            ChannelClass::Counter,
            ChannelClass::Feed,
        ] {
            assert_eq!(ChannelClass::try_from_str(class.as_str()).unwrap(), class);
        }
    }

    #[test]
    fn class_rejects_unknown() {
        let err = ChannelClass::try_from_str("mail").unwrap_err();
        assert_eq!(err.error_code(), "RTM-9001");
    }

    #[test]
    fn identity_scoping_and_delivery_are_consistent() {
        // Per-user classes are identity-scoped and at-least-once (notif/dm) …
        assert!(ChannelClass::Dm.is_identity_scoped());
        assert!(ChannelClass::Notification.is_identity_scoped());
        assert!(ChannelClass::Dm.ack_required());
        assert!(ChannelClass::Notification.ack_required());
        // … presence is identity-scoped but fire-and-forget …
        assert!(ChannelClass::Presence.is_identity_scoped());
        assert!(!ChannelClass::Presence.ack_required());
        // … public entity classes are neither scoped nor acked.
        assert!(!ChannelClass::Counter.is_identity_scoped());
        assert!(!ChannelClass::Feed.is_identity_scoped());
        assert!(!ChannelClass::Counter.ack_required());
    }

    #[test]
    fn channel_renders_class_colon_key() {
        let ch = ChannelRef::new(ChannelClass::Dm, ChannelKey::new("alice").unwrap());
        assert_eq!(ch.to_string(), "dm:alice");
    }

    #[test]
    fn channel_key_rejects_blank() {
        assert_eq!(
            ChannelKey::new("  ").unwrap_err().error_code(),
            "RTM-9002"
        );
    }
}
