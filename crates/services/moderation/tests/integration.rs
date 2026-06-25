//! Live, container-backed integration suite for the moderation service.
//!
//! Moderation owns three stores, so this suite boots a real **PostgreSQL** (the
//! decision/case system of record), a real **ScyllaDB** (the evidence history),
//! and a real **Redis** (the enforcement projection + screen corpus) via the
//! shared `test-support` harness, then drives the production composition root
//! ([`moderation::app::App::compose`]) end-to-end through the gRPC handler. The
//! only stubbed dependency is the `account` directory (an external boundary).
//!
//! Gated behind `integration-moderation` so the default `cargo test -p moderation`
//! stays hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p moderation --features integration-moderation -- --nocapture
//! ```
//!
//! Coverage:
//! - **lifecycle** — open → decide(Suspend) → enforcement state restricted →
//!   appeal → overturn reverses the enforcement, clears the projection, and the
//!   decision ledger holds both the original and the reversal (append-only).
//! - **screen** — a seeded known-bad hash blocks at the Plane C gate and records
//!   automated evidence; clean content allows.
//! - **ingest** — report ingestion is idempotent (deterministic case dedup), a
//!   self-report is rejected, and a classifier signal accrues onto the case.
#![cfg(feature = "integration-moderation")]

mod moderation_it;
