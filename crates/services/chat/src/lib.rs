//! Chat microservice.
//!
//! Implements the unified Conversation layer for a hyperscale short-form video
//! and social platform. A single [`domain::aggregate::Conversation`] aggregate
//! projects onto two physically independent runtime planes — the bounded,
//! full-duplex **Member Plane** and the unbounded, read-only **Audience Plane**
//! (the *Shadowing Pattern*). Topology (`Group` / `Channel`) is immutable;
//! `Visibility` (`Private` / `Public`) drives the polymorphic behaviour and the
//! lazy attachment of the Audience Plane.
//!
//! The crate follows the workspace hexagonal layout:
//! - [`domain`]: aggregates, value objects, and domain events (no I/O).
//! - [`application`]: CQRS command/query handlers and ports (to be implemented).
//! - [`infrastructure`]: ScyllaDB, Redis Cluster, gRPC, and Kafka adapters
//!   (to be implemented).
//! - [`config`]: environment-driven runtime configuration.
//! - [`error`]: the service-wide [`error::ChatError`] contract.

pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
