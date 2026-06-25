//! The moderation application layer — use-case orchestration over the domain and
//! ports.
//!
//! ## Handler shape
//! Like `auth`, the write use-cases are explicit application-service handlers
//! rather than `cqrs::CommandHandler`: they return rich outputs (a decision, an
//! enforcement, a screen verdict) that a `Result<(), E>` command cannot carry.
//! Each takes a [`cqrs::Envelope`] (so the `correlation_id` threads into the
//! emitted domain events) and an injected `now` for deterministic tests. The
//! genuinely read-only use-cases implement [`cqrs::QueryHandler`] and ride the
//! query bus.
//!
//! ## Ports
//! Every external dependency is an `async_trait` port in [`port`], injected as an
//! `Arc<dyn …>` at the composition root, so the handlers never name a concrete
//! adapter. In-memory fakes of every port back the unit tests.
//!
//! ## Durable-first ordering
//! Handlers persist to the system of record *first*, then publish the Plane B
//! event. The durable write is the source of truth; the event is the
//! denormalization notification.

pub mod command;
pub mod policy;
pub mod port;
pub mod query;

#[cfg(test)]
pub mod fakes;

pub use policy::ModerationPolicy;
