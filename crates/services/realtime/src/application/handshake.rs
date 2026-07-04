use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::application::port::{ConnectionLocation, ConnectionRegistry, TokenVerifier};
use crate::domain::{Connection, ConnectionId, NodeId};
use crate::error::RealtimeError;

/// Authenticates a WebSocket handshake and registers the resulting connection.
///
/// This is the one place the plane authenticates — verify the edge token once,
/// pin the resulting [`crate::domain::Session`] to a fresh [`Connection`], and
/// publish the connection's placement to the registry so the dispatcher can route
/// to it. Everything afterwards (subscribe, ack, deliver) trusts that pinned
/// identity and is never re-authenticated.
pub struct HandshakeHandler {
    verifier: Arc<dyn TokenVerifier>,
    registry: Arc<dyn ConnectionRegistry>,
    /// This gateway node's identity — every connection accepted here lives on it.
    node_id: NodeId,
    /// The per-connection subscription cap (protects node memory).
    subscription_cap: usize,
}

impl HandshakeHandler {
    pub fn new(
        verifier: Arc<dyn TokenVerifier>,
        registry: Arc<dyn ConnectionRegistry>,
        node_id: NodeId,
        subscription_cap: usize,
    ) -> Self {
        Self {
            verifier,
            registry,
            node_id,
            subscription_cap,
        }
    }

    /// Accept a handshake: verify `edge_token`, open a [`Connection`] under the
    /// server-assigned `connection_id`, and bind its placement in the registry.
    /// Returns the live connection for the gateway's per-socket task to own.
    ///
    /// Authentication failure (`RTM-1001` / `RTM-1002`) or a registry fault
    /// (`RTM-4001`) aborts the handshake — the socket is rejected, not half-open.
    pub async fn accept(
        &self,
        edge_token: &str,
        connection_id: ConnectionId,
        now: DateTime<Utc>,
    ) -> Result<Connection, RealtimeError> {
        let session = self.verifier.verify(edge_token, now).await?;

        let connection = Connection::open(
            connection_id,
            self.node_id.clone(),
            session.clone(),
            self.subscription_cap,
            now,
        );

        let location = ConnectionLocation {
            user_id: session.user_id,
            device_id: session.device_id,
            connection_id: connection.id().clone(),
            node_id: self.node_id.clone(),
        };
        self.registry.bind(&location).await?;

        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::application::port::ConnectionRegistry;
    use crate::domain::{ConnectionState, UserId};

    #[tokio::test]
    async fn accepts_valid_token_and_binds_the_connection() {
        let fx = Fixture::new();
        fx.verifier.seed_valid("good-token", "alice", "dev-1");

        let conn = fx
            .handshake()
            .accept("good-token", ConnectionId::new("conn-1").unwrap(), fx.now())
            .await
            .unwrap();

        assert_eq!(conn.state(), ConnectionState::Active);
        assert_eq!(conn.user_id().as_str(), "alice");
        // The placement is now resolvable by the dispatcher.
        let located = fx
            .registry
            .resolve(&UserId::new("alice").unwrap())
            .await
            .unwrap();
        assert_eq!(located.len(), 1);
        assert_eq!(located[0].connection_id.as_str(), "conn-1");
        assert_eq!(located[0].node_id, fx.node_id);
    }

    #[tokio::test]
    async fn rejects_an_invalid_token() {
        let fx = Fixture::new();
        let err = fx
            .handshake()
            .accept("bogus", ConnectionId::new("conn-1").unwrap(), fx.now())
            .await
            .unwrap_err();
        assert_eq!(err.error_code(), "RTM-1001");
        // Nothing was bound.
        assert!(
            fx.registry
                .resolve(&UserId::new("alice").unwrap())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
