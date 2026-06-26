use std::collections::HashSet;
use std::sync::Arc;

use crate::application::event::DeliverableEvent;
use crate::application::port::{ConnectionRegistry, EventSource, NodeChannel};
use crate::domain::NodeId;
use crate::error::RealtimeError;

/// The outcome of fanning one event out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanOutOutcome {
    /// How many distinct gateway nodes a *targeted* event was published to.
    pub nodes_published: usize,
    /// True when a *targeted* recipient had no live connection (a fail-open no-op).
    pub offline: bool,
    /// True when this was a public broadcast (published once to the fleet channel).
    pub broadcast: bool,
}

/// The dispatcher's core use case: resolve a recipient against the registry and
/// publish the event to each owning node (deduplicated, so a user with two
/// devices on the same node is published to once).
///
/// **Fail-open is the defining behaviour:** a recipient with no live connection
/// is a successful no-op, never an error — the message is durable upstream and the
/// client re-syncs on reconnect. Only a genuine *fabric* fault (registry or node
/// channel down → `RTM-4001` / `RTM-4002`, retryable) propagates, so the
/// `run_consumer` driver can retry without committing the offset.
pub struct FanOutHandler {
    registry: Arc<dyn ConnectionRegistry>,
    channel: Arc<dyn NodeChannel>,
}

impl FanOutHandler {
    pub fn new(registry: Arc<dyn ConnectionRegistry>, channel: Arc<dyn NodeChannel>) -> Self {
        Self { registry, channel }
    }

    pub async fn fan_out(
        &self,
        event: &DeliverableEvent,
    ) -> Result<FanOutOutcome, RealtimeError> {
        // Public broadcast (counter / feed): one publish to the fleet channel;
        // every node delivers to its local subscribers. Never "offline".
        let Some(recipient) = &event.recipient else {
            self.channel.broadcast(event).await?;
            return Ok(FanOutOutcome {
                nodes_published: 0,
                offline: false,
                broadcast: true,
            });
        };

        // Targeted (dm / notif / presence): resolve the recipient and hop to its
        // owning node(s).
        let locations = self.registry.resolve(recipient).await?;

        // Optionally restrict to a single device; otherwise every placement.
        let targets = locations.iter().filter(|loc| match &event.device_id {
            Some(device) => &loc.device_id == device,
            None => true,
        });

        let mut published_nodes: HashSet<NodeId> = HashSet::new();
        for loc in targets {
            // Dedupe by node: two devices on one node get a single publish.
            if published_nodes.insert(loc.node_id.clone()) {
                self.channel.publish(&loc.node_id, event).await?;
            }
        }

        if published_nodes.is_empty() {
            return Ok(FanOutOutcome {
                nodes_published: 0,
                offline: true,
                broadcast: false,
            });
        }
        Ok(FanOutOutcome {
            nodes_published: published_nodes.len(),
            offline: false,
            broadcast: false,
        })
    }
}

/// Drive the dispatcher: pull events from `source` and fan each one out until the
/// source is drained. This is the testable core of the fan-out worker; the
/// production binary (Phase 5) wraps the Kafka `run_consumer` runtime around the
/// same [`FanOutHandler::fan_out`] call (manual commit, backoff/jitter, DLQ).
pub async fn run_dispatch(
    source: &dyn EventSource,
    handler: &FanOutHandler,
) -> Result<(), RealtimeError> {
    while let Some(event) = source.next_event().await? {
        handler.fan_out(&event).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::application::port::ConnectionLocation;
    use crate::domain::{ChannelClass, ChannelKey, ChannelRef, ConnectionId, DeviceId, UserId};

    fn event_for(recipient: &str, device: Option<&str>) -> DeliverableEvent {
        DeliverableEvent {
            recipient: Some(UserId::new(recipient).unwrap()),
            device_id: device.map(|d| DeviceId::new(d).unwrap()),
            channel: ChannelRef::new(ChannelClass::Dm, ChannelKey::new(recipient).unwrap()),
            payload: b"hello".to_vec(),
            event_type: "chat.message".to_owned(),
            emitted_at: Fixture::fixed_now(),
            idempotency_key: "evt-1".to_owned(),
        }
    }

    fn broadcast_event() -> DeliverableEvent {
        DeliverableEvent {
            recipient: None,
            device_id: None,
            channel: ChannelRef::new(ChannelClass::Counter, ChannelKey::new("post-1").unwrap()),
            payload: b"{\"score\":9}".to_vec(),
            event_type: "counter.popularity".to_owned(),
            emitted_at: Fixture::fixed_now(),
            idempotency_key: "pop-1".to_owned(),
        }
    }

    fn location(user: &str, device: &str, conn: &str, node: &str) -> ConnectionLocation {
        ConnectionLocation {
            user_id: UserId::new(user).unwrap(),
            device_id: DeviceId::new(device).unwrap(),
            connection_id: ConnectionId::new(conn).unwrap(),
            node_id: crate::domain::NodeId::new(node).unwrap(),
        }
    }

    #[tokio::test]
    async fn publishes_to_each_owning_node() {
        let fx = Fixture::new();
        fx.registry.seed(location("alice", "phone", "c1", "node-1"));
        fx.registry.seed(location("alice", "tablet", "c2", "node-2"));

        let out = fx.fanout().fan_out(&event_for("alice", None)).await.unwrap();

        assert!(!out.offline);
        assert_eq!(out.nodes_published, 2);
        assert_eq!(fx.channel.publish_count(), 2);
    }

    #[tokio::test]
    async fn dedupes_multiple_devices_on_the_same_node() {
        let fx = Fixture::new();
        fx.registry.seed(location("alice", "phone", "c1", "node-1"));
        fx.registry.seed(location("alice", "web", "c2", "node-1"));

        let out = fx.fanout().fan_out(&event_for("alice", None)).await.unwrap();

        assert_eq!(out.nodes_published, 1); // one publish for the shared node
        assert_eq!(fx.channel.publish_count(), 1);
    }

    #[tokio::test]
    async fn offline_recipient_is_a_fail_open_noop() {
        let fx = Fixture::new();
        // Registry has nobody for "ghost".
        let out = fx.fanout().fan_out(&event_for("ghost", None)).await.unwrap();

        assert!(out.offline);
        assert_eq!(out.nodes_published, 0);
        assert_eq!(fx.channel.publish_count(), 0);
    }

    #[tokio::test]
    async fn device_targeted_event_reaches_only_that_device() {
        let fx = Fixture::new();
        fx.registry.seed(location("alice", "phone", "c1", "node-1"));
        fx.registry.seed(location("alice", "tablet", "c2", "node-2"));

        let out = fx
            .fanout()
            .fan_out(&event_for("alice", Some("tablet")))
            .await
            .unwrap();

        assert_eq!(out.nodes_published, 1);
        assert_eq!(fx.channel.published_to("node-2"), 1);
        assert_eq!(fx.channel.published_to("node-1"), 0);
    }

    #[tokio::test]
    async fn broadcast_publishes_once_to_the_fleet_channel() {
        let fx = Fixture::new();
        let out = fx.fanout().fan_out(&broadcast_event()).await.unwrap();

        assert!(out.broadcast);
        assert!(!out.offline);
        assert_eq!(fx.channel.broadcast_count(), 1);
        assert_eq!(fx.channel.publish_count(), 0); // no targeted node publish
    }

    #[tokio::test]
    async fn registry_fault_propagates_as_retryable() {
        let fx = Fixture::new();
        fx.registry.set_unavailable(true);

        let err = fx
            .fanout()
            .fan_out(&event_for("alice", None))
            .await
            .unwrap_err();
        assert_eq!(err.error_code(), "RTM-4001");
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn run_dispatch_drains_the_source() {
        let fx = Fixture::new();
        fx.registry.seed(location("alice", "phone", "c1", "node-1"));
        fx.source.push(event_for("alice", None));
        fx.source.push(event_for("alice", None));
        fx.source.push(event_for("ghost", None)); // offline → no-op, still consumed

        run_dispatch(fx.source.as_ref(), &fx.fanout()).await.unwrap();

        assert!(fx.source.is_drained());
        assert_eq!(fx.channel.publish_count(), 2);
    }
}
