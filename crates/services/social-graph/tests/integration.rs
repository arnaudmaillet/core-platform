//! Live, container-backed integration suite for the social-graph microservice.
//!
//! The whole binary is gated behind the `integration-social-graph` feature so the
//! default `cargo test -p social-graph` stays hermetic and Docker-free. Run the
//! live suite explicitly:
//!
//! ```text
//! cargo test -p social-graph --features integration-social-graph -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB and Redis containers via `test-support`, applies
//! the `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`social_graph::app::App`]) with an in-process
//! no-op event publisher:
//!
//! - **adjacency consistency** — a follow lands in both the `followers` and
//!   `following` adjacency tables, and concurrent follows of one target leave the
//!   two in agreement.
//! - **block overrides follow** — blocking severs an existing follow across the
//!   tables and gates any re-follow attempt.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-social-graph")]

mod social_graph_it;
