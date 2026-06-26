//! Outbound ports — the only contracts the application layer holds against the
//! outside world. Concrete adapters (the `auth-context` token verifier, the Redis
//! registry, the Redis Pub/Sub node hop, the Kafka event source + decode layer)
//! live in `infrastructure` (Phase 4) and are injected as `Arc<dyn …>` at the
//! composition root, so the handlers never name a concrete adapter. Each is an
//! `async_trait`; in-memory fakes back the unit tests.
//!
//! * [`TokenVerifier`] — the handshake authentication seam (consulted once).
//! * [`ConnectionRegistry`] — the routing fabric (`user → node(s)`); its empty
//!   resolve is the fail-open "offline" signal, not an error.
//! * [`NodeChannel`] — the last hop to the owning gateway node.
//! * [`EventSource`] — the dispatcher's upstream (Kafka) feed.

pub mod connection_registry;
pub mod event_source;
pub mod node_channel;
pub mod token_verifier;

pub use connection_registry::{ConnectionLocation, ConnectionRegistry};
pub use event_source::EventSource;
pub use node_channel::NodeChannel;
pub use token_verifier::TokenVerifier;
