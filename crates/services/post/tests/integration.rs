//! Live, container-backed integration suite for the post microservice.
//!
//! The whole binary is gated behind the `integration-post` feature so the default
//! `cargo test -p post` stays hermetic and Docker-free. Run the live suite
//! explicitly:
//!
//! ```text
//! cargo test -p post --features integration-post -- --nocapture
//! ```
//!
//! It boots an ephemeral ScyllaDB container via `test-support`, applies the
//! `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`post::app::App`]) with an in-process capturing
//! event publisher:
//!
//! - **dual-table consistency** — a write lands in both `posts` and
//!   `posts_by_profile`, and concurrent creates leave the two views in agreement.
//! - **lifecycle & events** — create → publish → delete transitions the status
//!   and emits the matching domain events.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-post")]

mod post_it;
