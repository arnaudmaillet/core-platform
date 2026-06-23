//! Live, container-backed integration suite for the engagement microservice.
//!
//! The whole binary is gated behind the `integration-engagement` feature so the
//! default `cargo test -p engagement` stays hermetic and Docker-free. Run the
//! live suite explicitly:
//!
//! ```text
//! cargo test -p engagement --features integration-engagement -- --nocapture
//! ```
//!
//! Engagement is Redis-primary: the reaction/view/share hot path is a single
//! atomic Redis round-trip, with ScyllaDB durability handled by background
//! write-behind workers. The suite therefore boots only an ephemeral **Redis**
//! container (no ScyllaDB, no Kafka) and drives the hot path through the
//! production composition root ([`engagement::app::App`]) with a no-op publisher:
//!
//! - **atomic reaction toggle** — an upsert is idempotent (a repeated reaction by
//!   the same profile does not double-count) and a remove zeroes it.
//! - **concurrent view counter** — concurrent view records sum exactly, proving
//!   the atomic Redis increment.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-engagement")]

mod engagement_it;
