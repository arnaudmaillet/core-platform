//! Live, container-backed integration suite for the comment microservice.
//!
//! The whole binary is gated behind the `integration-comment` feature so the
//! default `cargo test -p comment` stays hermetic and Docker-free. Run the live
//! suite explicitly:
//!
//! ```text
//! cargo test -p comment --features integration-comment -- --nocapture
//! ```
//!
//! It boots an ephemeral ScyllaDB container via `test-support`, applies the
//! `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`comment::app::App`]) with an in-process no-op
//! publisher:
//!
//! - **dual-table threading** — a comment and its reply are consistent across the
//!   canonical `comments` table and the `comments_by_post` thread index.
//! - **tombstone vs purge** — deleting a leaf comment purges it, while deleting a
//!   comment with active replies tombstones it (preserving the thread).
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-comment")]

mod comment_it;
