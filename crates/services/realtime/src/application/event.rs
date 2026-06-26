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
    /// The user the event is addressed to. The registry resolves this to the
    /// node(s) holding the user's live sockets.
    pub recipient: UserId,
    /// Restrict delivery to one device; `None` ⇒ all of the recipient's
    /// connections (the common case).
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
