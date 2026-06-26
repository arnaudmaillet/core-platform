//! `realtime` — the platform's **client-facing live delivery plane**: it
//! terminates millions of multiplexed, long-lived client connections, fans
//! internal events out to the exact device that should see them, and owns **no**
//! entity.
//!
//! This service is a System-of-**Connection / Delivery**, never a System of
//! Record. Every byte it forwards is already durable in its owning service —
//! `chat` persisted the message, `notification` persisted the badge, `counter`
//! holds the magnitude. If the entire plane vanished, **no data would be lost**:
//! clients reconnect and re-sync from those SoRs via a sequence token; it just
//! feels laggy. That single framing sets its boundaries:
//!
//! * it answers **"deliver this, live, to this connection"** — never "store
//!   this", "is this user allowed to read this content" (authorized upstream at
//!   emit time), or "what did they miss" (re-synced from the SoR on reconnect).
//! * it exists to **collapse the fleet's ad-hoc client streaming into one plane**:
//!   `chat` and `notification` each grew their own client-facing fan-out, which
//!   means multiple sockets and redundant heartbeats on one device. Realtime is
//!   the single horizontal connection plane — one multiplexed socket per device —
//!   and reduces those services to mere event *producers*.
//! * it is a **structural bulkhead**: millions of flaky mobile connections
//!   terminate here and never propagate into the internal gRPC mesh, which only
//!   ever sees a bounded set of stable gateway peers. Internal QPS tracks
//!   *events*, not idle eyeballs.
//!
//! The architectural commitment is a deliberate split into **two deployables**:
//! a **stateful edge gateway** (`realtime-gateway`, holds the WebSocket
//! connections + the connection registry) and a **stateless fan-out worker**
//! (`realtime-dispatcher`, consumes the upstream Kafka streams, resolves which
//! node owns a recipient, and publishes the event to that node). They scale on
//! different axes — connections-and-memory vs event-throughput — and share no
//! failure domain. Posture is **best-effort, fail-open**: a delivery miss costs
//! latency, never data. See `project_realtime_blueprint` for the full design.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `RTM-XXXX` namespace.
//! Phase 1: the client-facing transport **envelope** spec + the internal gRPC
//! contract (`realtime-api`). · Phase 2: `domain` (Connection, identity-pinned
//! Session, Subscription set + channel-scope authorization, the sequence/ack and
//! presence state machines — pure). · Phase 3: `application` + ports
//! (ConnectionRegistry / NodeChannel / TokenVerifier / EventSource + the
//! handshake, subscribe, deliver, reap and drain handlers + in-memory fakes). ·
//! Phase 4: `infrastructure` (the WebSocket server with bounded per-connection
//! mailboxes + shedding, the `fred` Redis registry, the Redis Pub/Sub node-hop,
//! the `auth-context` token verifier, the `run_consumer` event consumers). ·
//! Phase 5: `app` (composition roots) + `service` (the two runtime wirings — the
//! edge gateway and the fan-out dispatcher).

pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;

pub use error::RealtimeError;
pub use service::{RealtimeDispatcherService, RealtimeGatewayService};
