use std::sync::Arc;

use crate::application::port::ConnectionRegistry;
use crate::domain::{ConnectionId, UserId};
use crate::error::RealtimeError;

/// Tears down a connection's registry presence when it ends — whether by a clean
/// client disconnect, a heartbeat reap (`RTM-5002`), or a send-queue shed
/// (`RTM-5001`).
///
/// The matching domain transition (`Connection::close` / `begin_drain`) and the
/// reconnect control frame on a drain (`SERVER_CONTROL_RECONNECT`) are owned by
/// the gateway's per-socket task (domain + transport). This handler owns the one
/// *port* side effect of teardown: evicting the routing slot so the dispatcher
/// stops resolving the recipient to a node that no longer holds them. `evict` is
/// idempotent, so a double teardown (reap racing a disconnect) is harmless.
pub struct ReapHandler {
    registry: Arc<dyn ConnectionRegistry>,
}

impl ReapHandler {
    pub fn new(registry: Arc<dyn ConnectionRegistry>) -> Self {
        Self { registry }
    }

    pub async fn evict(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
    ) -> Result<(), RealtimeError> {
        self.registry.evict(user_id, connection_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::application::port::{ConnectionLocation, ConnectionRegistry};
    use crate::domain::{DeviceId, NodeId};

    fn location(user: &str, device: &str, conn: &str, node: &str) -> ConnectionLocation {
        ConnectionLocation {
            user_id: UserId::new(user).unwrap(),
            device_id: DeviceId::new(device).unwrap(),
            connection_id: ConnectionId::new(conn).unwrap(),
            node_id: NodeId::new(node).unwrap(),
        }
    }

    #[tokio::test]
    async fn evict_frees_the_routing_slot() {
        let fx = Fixture::new();
        fx.registry.seed(location("alice", "phone", "c1", "node-1"));
        fx.registry.seed(location("alice", "tablet", "c2", "node-1"));

        let alice = UserId::new("alice").unwrap();
        fx.reap()
            .evict(&alice, &ConnectionId::new("c1").unwrap())
            .await
            .unwrap();

        let remaining = fx.registry.resolve(&alice).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].connection_id.as_str(), "c2");
    }

    #[tokio::test]
    async fn evicting_an_absent_connection_is_idempotent() {
        let fx = Fixture::new();
        let alice = UserId::new("alice").unwrap();
        // Nothing seeded — eviction is still Ok.
        fx.reap()
            .evict(&alice, &ConnectionId::new("nope").unwrap())
            .await
            .unwrap();
    }
}
