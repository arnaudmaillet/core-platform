//! Live, container-backed integration suite for the profile microservice.
//!
//! The whole binary is gated behind the `integration-profile` feature so the
//! default `cargo test -p profile` stays hermetic and Docker-free. Run the live
//! suite explicitly:
//!
//! ```text
//! cargo test -p profile --features integration-profile -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB and Redis containers via `test-support`, applies
//! the `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`profile::app::App`]):
//!
//! - **handle-claim race** — concurrent creates of the same handle resolve to
//!   exactly one winner via the ScyllaDB LWT; the rest get `HandleAlreadyTaken`.
//! - **cache invalidation** — a read warms the Redis profile cache and a mutation
//!   busts it, so the next read reflects the new value.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-profile")]

mod profile_it;
