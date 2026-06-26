use async_trait::async_trait;

use crate::application::event::DeliverableEvent;
use crate::domain::NodeId;
use crate::error::RealtimeError;

/// The node-hop fabric: publishes a resolved event to the gateway node that owns
/// the recipient's socket (the last leg of the internal→external bridge).
///
/// LOCKED (see `project_realtime_blueprint`): v1 wires this as **Redis Pub/Sub**
/// — fire-and-forget, fail-open — publishing the event (as a
/// `realtime.v1.DeliverEnvelope`) on the owning node's `node:{node_id}` channel.
/// The `realtime.v1.RealtimeDispatchService` gRPC stream is the documented,
/// swappable alternative; this port's shape is fixed so the fabric can change
/// without touching the dispatcher.
///
/// A transport fault is `RTM-4002 NodeChannelUnavailable` (retryable). Note the
/// publish is intentionally *not* a delivery confirmation: the owning node may
/// have just lost the connection (stale registry), in which case the event is
/// silently dropped there — a fail-open miss the client recovers from on reconnect.
#[async_trait]
pub trait NodeChannel: Send + Sync + 'static {
    /// Targeted hop: publish to the channel of the one node that owns the
    /// recipient's socket(s).
    async fn publish(
        &self,
        node_id: &NodeId,
        event: &DeliverableEvent,
    ) -> Result<(), RealtimeError>;

    /// Public broadcast: publish to the fleet broadcast channel, from which
    /// *every* gateway node delivers the event to its local connections subscribed
    /// to the event's channel. Used for entity channels (counter / feed) that have
    /// no single recipient. Same fail-open posture as [`publish`](Self::publish)
    /// (`RTM-4002` on a transport fault).
    async fn broadcast(&self, event: &DeliverableEvent) -> Result<(), RealtimeError>;
}
