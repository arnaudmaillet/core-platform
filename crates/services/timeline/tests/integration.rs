//! Live, container-backed integration suite for the timeline microservice.
//!
//! The whole binary is gated behind the `integration-timeline` feature so the
//! default `cargo test -p timeline` stays hermetic and Docker-free. Run the live
//! suite explicitly:
//!
//! ```text
//! cargo test -p timeline --features integration-timeline -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB and Redis containers via `test-support`, applies
//! the `.cql` migrations (single-node RF=1), and drives the hybrid fan-out engine
//! through the production composition root ([`timeline::app::App`]) with an
//! in-process [`FakeSocialGraph`](timeline_it::fakes::FakeSocialGraph) standing in
//! for the social-graph gRPC dependency:
//!
//! - **fan-out ordering** — fan-out-on-write materializes a follower's Redis feed
//!   newest-first and enforces the cap.
//! - **following cache** — the following-set is rebuilt from social-graph once and
//!   then served from Redis (no repeat gRPC).
//! - **warm-up lifecycle** — a cold read serves from ScyllaDB and converges to a
//!   warm Redis feed; concurrent cold readers all get correct cold data.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-timeline")]

mod timeline_it;
