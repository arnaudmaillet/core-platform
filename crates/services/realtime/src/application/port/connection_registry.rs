use async_trait::async_trait;

use crate::domain::{ConnectionId, DeviceId, NodeId, UserId};
use crate::error::RealtimeError;

/// Where one live connection lives: the `(user, device, connection)` identity and
/// the gateway `node` holding its socket. The routing fabric is a set of these
/// keyed by user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionLocation {
    pub user_id: UserId,
    pub device_id: DeviceId,
    pub connection_id: ConnectionId,
    pub node_id: NodeId,
}

/// The connection/presence registry — the plane's only piece of state, and
/// deliberately ephemeral. It maps a `UserId` to the node(s) currently holding
/// that user's sockets, so the dispatcher can resolve *targeted* delivery instead
/// of broadcasting to every node.
///
/// The concrete adapter (Phase 4) is Redis (`fred`, hash-tag slot-safe, TTL'd so
/// a leaked entry self-heals). Semantics that matter:
/// * `resolve` returning an **empty** set is normal and benign — the recipient is
///   offline, and the caller treats it as a fail-open no-op (the event is durable
///   upstream; the client re-syncs on reconnect). It is *not* an error.
/// * a genuine store fault (Redis down) is `RTM-4001 RegistryUnavailable`
///   (retryable) — on the dispatcher path this drives the `run_consumer`
///   retry/DLQ classification.
#[async_trait]
pub trait ConnectionRegistry: Send + Sync + 'static {
    /// Register a newly-established connection's placement (at handshake).
    async fn bind(&self, location: &ConnectionLocation) -> Result<(), RealtimeError>;

    /// Remove a connection's placement (on clean disconnect or heartbeat reap),
    /// freeing its routing slot. Idempotent: evicting an absent entry is `Ok`.
    async fn evict(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
    ) -> Result<(), RealtimeError>;

    /// Resolve every live placement for a user. An empty result means "offline".
    async fn resolve(&self, user_id: &UserId) -> Result<Vec<ConnectionLocation>, RealtimeError>;
}
