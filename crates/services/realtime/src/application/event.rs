use chrono::{DateTime, Utc};

use crate::domain::{ChannelRef, DeviceId, UserId};

/// The application's unit of fan-out and delivery: one already-authorized event,
/// addressed to a recipient on a channel, carrying an opaque payload.
///
/// This is a transport-agnostic DTO — it is *not* a proto type. The dispatcher's
/// Kafka decode layer (Phase 4) builds it from an upstream event; the node-hop
/// adapter maps it to/from `realtime.v1.DeliverEnvelope`; the gateway turns it
/// into a `realtime.v1.Event` frame. The realtime plane never inspects `payload`
/// — it is bytes produced upstream by `chat` / `notification` / `counter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverableEvent {
    /// The delivery mode, by addressing:
    /// * `Some(user)` — **targeted**: an identity-scoped event (dm / notif /
    ///   presence). The registry resolves the user to the node(s) holding its
    ///   sockets, and the dispatcher hops to those nodes.
    /// * `None` — **public broadcast**: an entity-channel event (counter / feed)
    ///   with no single recipient. The dispatcher publishes it to the fleet
    ///   broadcast channel, and every node delivers it to its local connections
    ///   subscribed to [`channel`](Self::channel).
    pub recipient: Option<UserId>,
    /// Restrict a *targeted* delivery to one device; `None` ⇒ all of the
    /// recipient's connections. Ignored for a broadcast.
    pub device_id: Option<DeviceId>,
    pub channel: ChannelRef,
    /// Opaque upstream payload, forwarded verbatim — never interpreted or stored.
    pub payload: Vec<u8>,
    /// Routing/telemetry hint (e.g. `"chat.message"`); advisory, not a contract
    /// on `payload`'s schema.
    pub event_type: String,
    /// Event-time, propagated from the source event.
    pub emitted_at: DateTime<Utc>,
    /// Idempotency key from the source event, so a Kafka redelivery (the
    /// dispatcher runs at-least-once under `run_consumer`) does not double-emit on
    /// a client that already saw it.
    pub idempotency_key: String,
}

impl DeliverableEvent {
    /// Whether this is a public broadcast (no single recipient).
    pub fn is_broadcast(&self) -> bool {
        self.recipient.is_none()
    }
}
