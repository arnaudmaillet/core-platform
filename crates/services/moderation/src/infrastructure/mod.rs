//! The moderation infrastructure layer — concrete adapters implementing the
//! application ports against real backends.
//!
//! The three-store split:
//! * **`persistence`** — PostgreSQL adapters for the decision/case system of
//!   record (cases, decisions, enforcements, penalty ledgers, appeals).
//! * **`history`** — the ScyllaDB append-only evidence feed (an `EventPublisher`
//!   sink, composed alongside Kafka as fan-out).
//! * **`cache`** — Redis adapters for the Plane B enforcement projection and the
//!   Plane C screen corpus.
//!
//! Plus the event publisher (`event`), the classifier gateway stub
//! (`classifier`), and the `account` gRPC client (`directory`). The gRPC service
//! handler and the ingestion consumers are wired in Phase 5.

pub mod cache;
pub mod classifier;
pub mod consumer;
pub mod directory;
pub mod event;
pub mod grpc;
pub mod history;
pub mod persistence;
