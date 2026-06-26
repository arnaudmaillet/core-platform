use serde::{Deserialize, Serialize};

use crate::error::RealtimeError;

/// Declares an opaque, non-empty `String` newtype used as a domain identifier.
/// A blank value is a malformed input, surfaced as `RTM-9002 InvalidIdentifier`
/// with the field name. The realtime plane never interprets these beyond equality
/// and key composition — they are references to subjects owned elsewhere.
macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, RealtimeError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(RealtimeError::InvalidIdentifier($field.to_owned()));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// The authenticated end-user a connection is pinned to (from the verified
    /// edge token). The authorization subject: identity-scoped channel keys must
    /// equal this.
    UserId, "user_id"
);

string_id!(
    /// The specific device/installation behind a connection. A single `UserId`
    /// may hold several connections (phone + tablet + web), each a distinct
    /// `DeviceId`.
    DeviceId, "device_id"
);

string_id!(
    /// A single live connection, server-assigned at the handshake. Unique within
    /// a gateway node for the connection's lifetime.
    ConnectionId, "connection_id"
);

string_id!(
    /// A gateway node in the edge pool. The registry maps a `UserId` to the
    /// `NodeId`(s) holding its sockets; the dispatcher publishes to that node's
    /// hop channel.
    NodeId, "node_id"
);

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn accepts_non_empty() {
        assert_eq!(UserId::new("alice").unwrap().as_str(), "alice");
        assert_eq!(DeviceId::new("dev-1").unwrap().as_str(), "dev-1");
        assert_eq!(ConnectionId::new("conn-1").unwrap().as_str(), "conn-1");
        assert_eq!(NodeId::new("node-7").unwrap().as_str(), "node-7");
    }

    #[test]
    fn rejects_blank_with_field_named_code() {
        let err = UserId::new("   ").unwrap_err();
        assert_eq!(err.error_code(), "RTM-9002");
        assert!(err.to_string().contains("user_id"));

        assert_eq!(NodeId::new("").unwrap_err().error_code(), "RTM-9002");
    }
}
