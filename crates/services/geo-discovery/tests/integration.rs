//! Live, container-backed integration suite for the geo-discovery microservice.
//!
//! The whole binary is gated behind the `integration-geo-discovery` feature so
//! the default `cargo test -p geo-discovery` stays hermetic and Docker-free. Run
//! the live suite explicitly:
//!
//! ```text
//! cargo test -p geo-discovery --features integration-geo-discovery -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB and Redis containers via `test-support`, applies
//! the `.cql` migrations (single-node RF=1), and drives the service through the
//! production composition root ([`geo_discovery::app::App`]) — indexing posts via
//! the command bus (no Kafka) and reading them back via the viewport query:
//!
//! - **viewport query** — posts indexed at a coordinate are returned by a viewport
//!   that covers them, hydrated from the spatial index + card store / ScyllaDB.
//! - **spatial filtering** — a viewport over a distant region excludes them.
//!
//! Queries use zoom 15 (H3 R9, virality floor 0) so the spatial filter — not a
//! score threshold — is what's under test. All cross-component synchronisation
//! polls observable state with a deadline (`await_until`); there are no fixed
//! sleeps.
#![cfg(feature = "integration-geo-discovery")]

mod geo_it;
