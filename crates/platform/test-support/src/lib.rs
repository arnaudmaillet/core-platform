//! Shared integration-test scaffolding for the core-platform services.
//!
//! This crate is the backend-agnostic backbone every service's live suite is
//! built on. It owns the parts that are *identical* across services so each
//! service's `tests/<svc>_it/` directory carries only what is irreducibly
//! service-specific (the composition-root graph and the scenarios themselves).
//!
//! # The five pillars (extracted from the `chat` gold standard)
//!
//! - **One container set per test binary.** Each backend boots lazily through a
//!   [`tokio::sync::OnceCell`] and is shared by every scenario in the binary
//!   (see [`containers`]). Kafka/Postgres boot only when a scenario first asks.
//! - **Zero port conflicts.** Every endpoint is resolved from the OS-assigned
//!   mapped host port; nothing is statically bound.
//! - **Migrations applied exactly once**, behind a `OnceCell`, with the
//!   single-node replication adaptation (ScyllaDB `SimpleStrategy RF=1`) or a
//!   raw-SQL runner (Postgres) — see [`migrate`].
//! - **Zero fixed sleeps.** [`await_until`] is the single synchronisation
//!   primitive: assertions poll observable state with a deadline, never sleep a
//!   fixed amount.
//! - **Isolation by namespacing, not teardown.** Scenarios mint fresh UUID keys
//!   so the suite runs in parallel against the shared containers; this crate
//!   only provides the infra, the namespacing discipline lives in each harness.

pub mod containers;
pub mod migrate;
pub mod wait;

pub use wait::await_until;
