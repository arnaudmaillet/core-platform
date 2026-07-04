use error::{AppError, Severity};
use http::StatusCode;
use thiserror::Error;

/// Canonical domain and application error type for the realtime delivery
/// microservice.
///
/// The `RTM-XXXX` namespace is grouped by concern so a code alone localizes the
/// fault: 1xxx connection handshake / authentication, 2xxx transport / framing /
/// protocol, 3xxx subscription authorization (the only authz the plane performs
/// — channel-scope ownership), 4xxx delivery-fabric availability (the fail-open
/// core: registry + node-hop), 5xxx connection lifecycle / backpressure, 8xxx
/// inbound event decode / routing (the dispatcher's Kafka surface), 9xxx
/// cross-cutting (domain/parse, event consumption).
///
/// ## Code catalogue
///
/// | Code     | Variant                     | HTTP | Severity | Retryable |
/// |----------|-----------------------------|------|----------|-----------|
/// | RTM-1001 | HandshakeRejected           | 401  | Medium   | No        |
/// | RTM-1002 | TokenExpired                | 401  | Low      | No        |
/// | RTM-1003 | UnsupportedProtocolVersion  | 426  | Low      | No        |
/// | RTM-1004 | OriginNotAllowed            | 403  | Medium   | No        |
/// | RTM-2001 | MalformedFrame              | 400  | Low      | No        |
/// | RTM-2002 | UnknownChannel              | 422  | Low      | No        |
/// | RTM-2003 | FrameTooLarge               | 413  | Low      | No        |
/// | RTM-2004 | SequenceViolation           | 422  | Low      | No        |
/// | RTM-3001 | ChannelForbidden            | 403  | **High** | No        |
/// | RTM-3002 | SubscriptionLimitExceeded   | 429  | Low      | No        |
/// | RTM-3003 | NotSubscribed               | 409  | Low      | No        |
/// | RTM-4001 | RegistryUnavailable         | 503  | **High** | **Yes**   |
/// | RTM-4002 | NodeChannelUnavailable      | 503  | **High** | **Yes**   |
/// | RTM-4003 | DeliveryTimeout             | 504  | **High** | **Yes**   |
/// | RTM-5001 | SendQueueOverflow           | 503  | Medium   | No        |
/// | RTM-5002 | HeartbeatTimeout            | 408  | Low      | No        |
/// | RTM-5003 | ConnectionDraining          | 503  | Low      | **Yes**   |
/// | RTM-5004 | ConnectionClosed            | 410  | Low      | No        |
/// | RTM-8001 | EventDecodeFailed           | 422  | Medium   | No        |
/// | RTM-8002 | UnroutableEvent             | 422  | Low      | No        |
/// | RTM-8003 | UnknownEventType            | 422  | Low      | No        |
/// | RTM-9001 | DomainViolation             | 422  | Medium   | No        |
/// | RTM-9002 | InvalidIdentifier           | 422  | Low      | No        |
/// | RTM-9003 | EventConsumeFailed          | 500  | Medium   | No        |
/// | VAL-*    | Validation (delegated)      | 422  | Low      | No        |
///
/// > **Fail-open semantics.** Realtime is a best-effort *delivery* plane, never a
/// > System of Record. The `4xxx` delivery-fabric faults are *transient*: on the
/// > **client** path a delivery miss degrades silently — the message is durable in
/// > its owning SoR (`chat` / `notification`) and the client re-syncs on reconnect
/// > via its sequence token, never a hard error; on the **dispatcher** (Kafka)
/// > path their `is_retryable` flags drive the `run_consumer` retry/DLQ
/// > classification — a redelivered event re-resolves the registry and re-fans-out
/// > idempotently, a poison event (`RTM-8001`) is dead-lettered, and an
/// > unroutable/unknown event (`RTM-8002` / `RTM-8003`) is a harmless skip folded
/// > into `Ok` so the offset still commits. The plane stores nothing; losing a
/// > live event costs latency, never data.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RealtimeError {
    // ── Delegated ─────────────────────────────────────────────────────────────
    #[error(transparent)]
    Validation(#[from] validation::ValidationError),

    // ── Connection handshake / authentication (RTM-1xxx) ──────────────────────
    /// The WebSocket upgrade presented a missing, malformed, or invalid edge
    /// token. Authentication happens **once**, at the handshake — never per frame.
    #[error("connection handshake rejected: {reason}")]
    HandshakeRejected { reason: String },

    /// The session's edge token lapsed mid-connection; the client must silently
    /// refresh via `auth` and re-handshake.
    #[error("session token expired")]
    TokenExpired,

    #[error("unsupported client protocol version: '{version}'")]
    UnsupportedProtocolVersion { version: String },

    #[error("connection origin not allowed: '{origin}'")]
    OriginNotAllowed { origin: String },

    // ── Transport / framing / protocol (RTM-2xxx) ─────────────────────────────
    #[error("malformed transport frame: {reason}")]
    MalformedFrame { reason: String },

    #[error("unknown multiplex channel: '{channel}'")]
    UnknownChannel { channel: String },

    #[error("inbound frame exceeds the size cap: {size} bytes")]
    FrameTooLarge { size: usize },

    #[error("resume/ack sequence out of range: {reason}")]
    SequenceViolation { reason: String },

    // ── Subscription authorization · the plane's only authz (RTM-3xxx) ────────
    /// A connection tried to subscribe to a channel not scoped to its pinned
    /// identity — i.e. an attempt to tap someone else's stream. Security-relevant.
    #[error("subscription to channel '{channel}' is forbidden for this identity")]
    ChannelForbidden { channel: String },

    #[error("per-connection subscription limit exceeded")]
    SubscriptionLimitExceeded,

    #[error("not subscribed to channel '{channel}'")]
    NotSubscribed { channel: String },

    // ── Delivery fabric availability · the fail-open core (RTM-4xxx) ──────────
    /// The connection/presence registry (Redis) is unavailable — the dispatcher
    /// cannot resolve which node owns a recipient's socket.
    #[error("the connection registry is unavailable")]
    RegistryUnavailable,

    /// The node-hop fabric (Redis Pub/Sub) is unavailable — a resolved event
    /// cannot be published to the owning gateway node.
    #[error("the node-delivery channel is unavailable")]
    NodeChannelUnavailable,

    #[error("delivery to the owning node timed out")]
    DeliveryTimeout,

    // ── Connection lifecycle / backpressure (RTM-5xxx) ────────────────────────
    /// The per-connection send queue is full; the slow consumer is shed rather
    /// than allowed to balloon node memory. Not retryable in place.
    #[error("connection send queue overflowed; shedding slow consumer")]
    SendQueueOverflow,

    /// No heartbeat pong arrived within the deadline; the half-open connection is
    /// reaped, freeing its file descriptor and registry slot.
    #[error("heartbeat deadline exceeded; reaping connection")]
    HeartbeatTimeout,

    /// The node is draining on rollout; the client should reconnect elsewhere
    /// with jittered backoff. Retryable from the client's perspective.
    #[error("connection draining; reconnect with backoff")]
    ConnectionDraining,

    #[error("connection closed by peer")]
    ConnectionClosed,

    // ── Inbound event decode / routing · dispatcher surface (RTM-8xxx) ────────
    #[error("failed to decode event from topic '{topic}': {reason}")]
    EventDecodeFailed { topic: String, reason: String },

    /// The event carries no addressable recipient (no live target); a harmless
    /// skip folded into `Ok` so the offset still commits.
    #[error("event has no routable recipient: {reason}")]
    UnroutableEvent { reason: String },

    /// An event type this dispatcher does not fan out; folded into an `Ok` skip
    /// rather than dead-lettered.
    #[error("unknown event type: '{event_type}'")]
    UnknownEventType { event_type: String },

    // ── Cross-cutting (RTM-9xxx) ──────────────────────────────────────────────
    #[error("domain invariant violated on '{field}': {message}")]
    DomainViolation { field: String, message: String },

    #[error("invalid identifier: '{0}'")]
    InvalidIdentifier(String),

    #[error("failed to consume event: {0}")]
    EventConsumeFailed(String),
}

impl AppError for RealtimeError {
    fn error_code(&self) -> &'static str {
        match self {
            RealtimeError::Validation(e) => e.error_code(),

            RealtimeError::HandshakeRejected { .. } => "RTM-1001",
            RealtimeError::TokenExpired => "RTM-1002",
            RealtimeError::UnsupportedProtocolVersion { .. } => "RTM-1003",
            RealtimeError::OriginNotAllowed { .. } => "RTM-1004",

            RealtimeError::MalformedFrame { .. } => "RTM-2001",
            RealtimeError::UnknownChannel { .. } => "RTM-2002",
            RealtimeError::FrameTooLarge { .. } => "RTM-2003",
            RealtimeError::SequenceViolation { .. } => "RTM-2004",

            RealtimeError::ChannelForbidden { .. } => "RTM-3001",
            RealtimeError::SubscriptionLimitExceeded => "RTM-3002",
            RealtimeError::NotSubscribed { .. } => "RTM-3003",

            RealtimeError::RegistryUnavailable => "RTM-4001",
            RealtimeError::NodeChannelUnavailable => "RTM-4002",
            RealtimeError::DeliveryTimeout => "RTM-4003",

            RealtimeError::SendQueueOverflow => "RTM-5001",
            RealtimeError::HeartbeatTimeout => "RTM-5002",
            RealtimeError::ConnectionDraining => "RTM-5003",
            RealtimeError::ConnectionClosed => "RTM-5004",

            RealtimeError::EventDecodeFailed { .. } => "RTM-8001",
            RealtimeError::UnroutableEvent { .. } => "RTM-8002",
            RealtimeError::UnknownEventType { .. } => "RTM-8003",

            RealtimeError::DomainViolation { .. } => "RTM-9001",
            RealtimeError::InvalidIdentifier(_) => "RTM-9002",
            RealtimeError::EventConsumeFailed(_) => "RTM-9003",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            RealtimeError::Validation(e) => e.http_status(),

            RealtimeError::HandshakeRejected { .. } | RealtimeError::TokenExpired => {
                StatusCode::UNAUTHORIZED
            }
            RealtimeError::UnsupportedProtocolVersion { .. } => StatusCode::UPGRADE_REQUIRED,
            RealtimeError::OriginNotAllowed { .. } | RealtimeError::ChannelForbidden { .. } => {
                StatusCode::FORBIDDEN
            }

            RealtimeError::MalformedFrame { .. } => StatusCode::BAD_REQUEST,
            RealtimeError::FrameTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            RealtimeError::SubscriptionLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            RealtimeError::NotSubscribed { .. } => StatusCode::CONFLICT,

            RealtimeError::RegistryUnavailable
            | RealtimeError::NodeChannelUnavailable
            | RealtimeError::SendQueueOverflow
            | RealtimeError::ConnectionDraining => StatusCode::SERVICE_UNAVAILABLE,

            RealtimeError::DeliveryTimeout => StatusCode::GATEWAY_TIMEOUT,
            RealtimeError::HeartbeatTimeout => StatusCode::REQUEST_TIMEOUT,
            RealtimeError::ConnectionClosed => StatusCode::GONE,

            RealtimeError::EventConsumeFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,

            _ => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            RealtimeError::Validation(e) => e.severity(),

            RealtimeError::ChannelForbidden { .. }
            | RealtimeError::RegistryUnavailable
            | RealtimeError::NodeChannelUnavailable
            | RealtimeError::DeliveryTimeout => Severity::High,

            RealtimeError::HandshakeRejected { .. }
            | RealtimeError::OriginNotAllowed { .. }
            | RealtimeError::SendQueueOverflow
            | RealtimeError::EventDecodeFailed { .. }
            | RealtimeError::DomainViolation { .. }
            | RealtimeError::EventConsumeFailed(_) => Severity::Medium,

            _ => Severity::Low,
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            RealtimeError::Validation(e) => e.is_retryable(),
            RealtimeError::RegistryUnavailable
            | RealtimeError::NodeChannelUnavailable
            | RealtimeError::DeliveryTimeout
            | RealtimeError::ConnectionDraining => true,
            _ => false,
        }
    }

    fn category(&self) -> &'static str {
        match self {
            RealtimeError::Validation(e) => e.category(),
            _ => "RTM",
        }
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            RealtimeError::Validation(e) => e.user_facing_message(),

            RealtimeError::HandshakeRejected { .. }
            | RealtimeError::TokenExpired
            | RealtimeError::OriginNotAllowed { .. } => {
                "Your realtime session could not be authenticated."
            }

            RealtimeError::ChannelForbidden { .. } => {
                "You are not allowed to subscribe to that channel."
            }

            RealtimeError::RegistryUnavailable
            | RealtimeError::NodeChannelUnavailable
            | RealtimeError::DeliveryTimeout
            | RealtimeError::ConnectionDraining => {
                "Live updates are temporarily unavailable. Reconnecting…"
            }

            _ => "A realtime delivery error occurred.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant must carry a stable, correctly-prefixed `RTM-XXXX` code and
    /// agree with the documented retry classification that drives `run_consumer`
    /// on the dispatcher and the fail-open degradation on the client path.
    #[test]
    fn codes_are_stable_and_prefixed() {
        // Fail-open delivery-fabric fault → retryable, drives consumer retry.
        let registry_down = RealtimeError::RegistryUnavailable;
        assert_eq!(registry_down.error_code(), "RTM-4001");
        assert!(registry_down.is_retryable());
        assert_eq!(
            registry_down.http_status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(registry_down.severity(), Severity::High);
        assert_eq!(registry_down.category(), "RTM");

        // Security-relevant authz denial — the plane's only authorization check.
        let forbidden = RealtimeError::ChannelForbidden {
            channel: "dm:bob".into(),
        };
        assert_eq!(forbidden.error_code(), "RTM-3001");
        assert_eq!(forbidden.http_status(), StatusCode::FORBIDDEN);
        assert_eq!(forbidden.severity(), Severity::High);
        assert!(!forbidden.is_retryable());

        // Poison upstream event → DLQ, never an infinite retry.
        let poison = RealtimeError::EventDecodeFailed {
            topic: "notification.v1.events".into(),
            reason: "bad frame".into(),
        };
        assert_eq!(poison.error_code(), "RTM-8001");
        assert!(!poison.is_retryable());

        // Unroutable / unknown events are folded into Ok skips at the consumer.
        let skip = RealtimeError::UnroutableEvent {
            reason: "recipient offline".into(),
        };
        assert_eq!(skip.error_code(), "RTM-8002");
        assert!(!skip.is_retryable());
    }
}
