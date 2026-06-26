//! Live, container-backed integration suite for the audit & compliance service.
//!
//! Audit's whole point is a tamper-evident, append-only ledger with crypto-shred
//! erasure, so this suite boots real **Postgres** (the canonical hash-chained
//! ledger + the per-subject key vault + the checkpoint anchors) and real **MinIO**
//! (the Object-Lock WORM archive), and drives the production write/verify path over
//! the real adapters. It exercises exactly what cannot be unit-tested: that the
//! compare-and-append + `event_id` unique constraint behave as the domain assumes,
//! that a rogue in-place `UPDATE` (a privileged operator bypassing the INSERT-only
//! grant) is caught by the verifier, that destroying a DEK leaves the chain intact,
//! and that the checkpoint round-trips through the real anchor.
//!
//! The async Kafka ingest *runtime* (`run_consumer`) is covered by `transport`'s own
//! live suite; here ingestion is driven through the real `IngestHandler` so the
//! storage adapters are what's under test.
//!
//! Gated behind `integration-audit` so the default `cargo test -p audit` stays
//! hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p audit --features integration-audit -- --nocapture
//! ```
#![cfg(feature = "integration-audit")]

mod audit_it;
