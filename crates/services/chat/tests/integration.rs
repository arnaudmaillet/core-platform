//! Live, container-backed integration suite for the chat microservice.
//!
//! The whole binary is gated behind the `integration-chat` feature so the
//! default `cargo test -p chat` stays hermetic and Docker-free. Run the live
//! suite explicitly:
//!
//! ```text
//! cargo test -p chat --features integration-chat -- --nocapture
//! ```
//!
//! It boots ephemeral ScyllaDB, Redis (7.x — sharded pub/sub), and Kafka
//! containers via testcontainers, applies the six `.cql` migrations, and
//! empirically validates the Shadowing Pattern's runtime behaviour:
//!
//! - **Scenario 1** — the privacy boundary: the Audience Plane receives message
//!   shadows but is structurally shielded from Member-Plane noise.
//! - **Scenario 2** — RAII stream-leak protection: a dropped stream releases
//!   presence, the refcounted Redis subscription, and shard activation.
//! - **Scenario 3** — backpressure & data-loss recovery: a starved stream gets a
//!   controlled `data_loss`, the pod stays healthy, and the client repages from
//!   ScyllaDB.
//! - **Scenario 4** — Kafka-driven Audience-Plane teardown on unpublish.
//!
//! All cross-component synchronisation polls durable observable state with a
//! deadline (`await_until` / bounded stream `recv`); there are no fixed sleeps.
#![cfg(feature = "integration-chat")]

mod chat_it;
