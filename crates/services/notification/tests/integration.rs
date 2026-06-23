//! Live, container-backed integration suite for the notification microservice.
//!
//! The whole binary is gated behind the `integration-notification` feature so the
//! default `cargo test -p notification` stays hermetic and Docker-free. Run the
//! live suite explicitly:
//!
//! ```text
//! cargo test -p notification --features integration-notification -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB and Redis containers via `test-support`, applies
//! the `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`notification::app::App`]):
//!
//! - **stream lifetime** — a gRPC broadcast stream delivers a created
//!   notification, and dropping it reclaims the registry sender; the sender is
//!   refcounted so it survives until the last subscriber leaves.
//! - **unread counter** — concurrent creates produce an exact unread count, and
//!   the claim-gated `increment_once` is idempotent under a concurrent stampede.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until` / bounded stream `recv`); there are no fixed sleeps.
#![cfg(feature = "integration-notification")]

mod notification_it;
