//! Infrastructure adapters — the concrete implementations of the application
//! ports, plus the proto codec, the inbound decode layer, and the per-connection
//! backpressure primitive.
//!
//! * [`auth_context_verifier`] — [`TokenVerifier`](crate::application::TokenVerifier)
//!   over the shared `auth-context` JWT decoder.
//! * [`redis_connection_registry`] — the routing fabric (fred, Lua-issued
//!   single-key writes + `HGETALL`, TTL self-heal).
//! * [`redis_node_channel`] — the node hop (fred sharded `SPUBLISH`).
//! * [`codec`] — the pure `realtime.v1` proto mapping (frames + node envelope).
//! * [`decode`] — realtime-owned wire DTOs + the pure `map_*` distillation into a
//!   [`DeliverableEvent`](crate::application::DeliverableEvent).
//! * [`mailbox`] — the bounded, drop-oldest per-connection send queue.
//! * [`consumer`] — the `run_consumer` error-classification glue.
//!
//! Per the integration-test standard, the Redis / auth adapters are compile-checked
//! here; their live behaviour is exercised by the gated suite in Phase 6. The
//! codec, decode, and mailbox layers are pure and unit-tested in place.

pub mod auth_context_verifier;
pub mod codec;
pub mod consumer;
pub mod decode;
pub mod mailbox;
pub mod redis_connection_registry;
pub mod redis_node_channel;
pub mod runtime;

pub use auth_context_verifier::AuthContextTokenVerifier;
pub use codec::ClientIntent;
pub use mailbox::{ConnectionMailbox, EnqueueOutcome};
pub use redis_connection_registry::RedisConnectionRegistry;
pub use redis_node_channel::RedisNodeChannel;
