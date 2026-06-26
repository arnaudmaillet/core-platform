use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::Session;
use crate::error::RealtimeError;

/// Verifies the edge token presented at the WebSocket handshake and yields the
/// pinned [`Session`] (the authenticated `user_id` + `device_id` + token expiry).
///
/// This is the plane's authentication seam. It is consulted exactly **once**, at
/// the handshake — never per frame. The concrete adapter (Phase 4) verifies the
/// ES256 edge token via the shared `auth-context` library; an in-memory fake
/// backs the unit tests.
///
/// A missing / malformed / signature-invalid token is `RTM-1001
/// HandshakeRejected`; a well-formed but already-expired token is `RTM-1002
/// TokenExpired`.
#[async_trait]
pub trait TokenVerifier: Send + Sync + 'static {
    async fn verify(
        &self,
        edge_token: &str,
        now: DateTime<Utc>,
    ) -> Result<Session, RealtimeError>;
}
