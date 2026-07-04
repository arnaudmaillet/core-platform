//! The realtime application layer — use-case orchestration over the domain and
//! the ports.
//!
//! ## Handler shape
//! The realtime plane has no synchronous read/query RPC (its internal gRPC is
//! health + the node-hop `DeliverToNode`), so there is no query/command bus here.
//! Use cases are plain application-service structs holding their ports as
//! `Arc<dyn …>`:
//! * [`HandshakeHandler`] — verify the edge token once, open a connection, bind
//!   it in the registry (the gateway's accept path).
//! * [`FanOutHandler`] + [`run_dispatch`] — resolve a recipient and publish to its
//!   owning node(s); the dispatcher's core, fail-open on an offline recipient.
//! * [`ReapHandler`] — evict a connection's routing slot on teardown.
//!
//! Subscribe / unsubscribe / ack are pure domain operations on
//! [`crate::domain::Connection`] invoked directly by the gateway's per-socket
//! task, so they need no port and no handler here.
//!
//! ## Ports
//! Every external dependency is an `async_trait` port in [`port`], injected at the
//! composition root. In-memory fakes back the unit tests; the live adapters are
//! Phase 4.

pub mod event;
pub mod fanout;
pub mod handshake;
pub mod lifecycle;
pub mod port;

#[cfg(test)]
pub mod fakes;

pub use event::DeliverableEvent;
pub use fanout::{FanOutHandler, FanOutOutcome, run_dispatch};
pub use handshake::HandshakeHandler;
pub use lifecycle::ReapHandler;
pub use port::{
    ConnectionLocation, ConnectionRegistry, EventSource, NodeChannel, TokenVerifier,
};
