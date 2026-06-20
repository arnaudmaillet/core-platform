use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Wraps any Command or Query payload with the distributed-tracing and
/// idempotency metadata that must travel with every message dispatched
/// through the bus.
///
/// ## Field contract
///
/// | Field            | Responsibility                                                   |
/// |------------------|------------------------------------------------------------------|
/// | `message_id`     | Unique per message instance — used as the idempotency key.       |
/// | `correlation_id` | Propagated across the full request flow (gRPC → bus → handler). |
/// | `causation_id`   | The `message_id` of the upstream message that triggered this one.|
/// | `issued_at`      | Wall-clock creation time (UTC).                                  |
/// | `metadata`       | Open bag: OTel context bytes, tenant IDs, feature flags, etc.   |
/// | `payload`        | The command or query value itself.                               |
///
/// ## Construction
///
/// - [`Envelope::new`] — starts a fresh causal chain.
/// - [`Envelope::new_caused_by`] — continues an existing chain, propagating
///   the parent's `correlation_id` and recording its `message_id` as
///   `causation_id`.
///
/// ## Relation to `EventEnvelope<T>` in the transport crate
///
/// `Envelope<T>` is the application-layer mirror of `transport::EventEnvelope<T>`.
/// When a Kafka consumer or gRPC handler extracts a `correlation_id` from an
/// inbound message, it should construct `Envelope::new(correlation_id, payload)`
/// so the same trace thread is preserved end-to-end.
#[derive(Debug, Clone)]
pub struct Envelope<T> {
    /// Unique identifier for this specific message. Used by [`IdempotencyLayer`]
    /// to detect and skip duplicate dispatches.
    pub message_id: Uuid,

    /// Identifier shared by every message produced within the same top-level
    /// request. Thread this from inbound transport headers into every envelope
    /// you create during request handling.
    pub correlation_id: Uuid,

    /// The `message_id` of the upstream message that caused this one, if any.
    pub causation_id: Option<Uuid>,

    /// Wall-clock time at which this envelope was constructed.
    pub issued_at: DateTime<Utc>,

    /// Arbitrary key-value pairs for cross-cutting concerns that do not
    /// warrant a dedicated field (e.g. serialised W3C TraceContext bytes,
    /// tenant ID, A/B test cohort).
    pub metadata: HashMap<String, String>,

    /// The command or query payload.
    pub payload: T,
}

impl<T> Envelope<T> {
    /// Creates a new envelope with a fresh UUIDv7 `message_id` and `issued_at = Utc::now()`.
    ///
    /// Use this at the top of a request boundary (e.g. a gRPC endpoint handler)
    /// where you already hold the inbound `correlation_id`.
    pub fn new(correlation_id: Uuid, payload: T) -> Self {
        Self {
            message_id: Uuid::now_v7(),
            correlation_id,
            causation_id: None,
            issued_at: Utc::now(),
            metadata: HashMap::new(),
            payload,
        }
    }

    /// Creates a new envelope caused by a parent envelope.
    ///
    /// Propagates the parent's `correlation_id` and records the parent's
    /// `message_id` as `causation_id`. Also inherits the parent's `metadata`
    /// so context values (e.g. tenant ID) flow through causal chains.
    pub fn new_caused_by<P>(parent: &Envelope<P>, payload: T) -> Self {
        Self {
            message_id: Uuid::now_v7(),
            correlation_id: parent.correlation_id,
            causation_id: Some(parent.message_id),
            issued_at: Utc::now(),
            metadata: parent.metadata.clone(),
            payload,
        }
    }

    /// Attaches a metadata entry and returns `self` for chaining.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Transforms the payload while preserving all envelope metadata.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Envelope<U> {
        Envelope {
            message_id: self.message_id,
            correlation_id: self.correlation_id,
            causation_id: self.causation_id,
            issued_at: self.issued_at,
            metadata: self.metadata,
            payload: f(self.payload),
        }
    }
}
