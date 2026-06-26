//! Live, container-backed integration suite for the counter-analytics service.
//!
//! Counter's whole point is absorbing a firehose across three storage tiers, so
//! this suite boots real **Redis** (hot counters / HLL / trending), **Postgres**
//! (the durable ledger + idempotency), and **ScyllaDB** (the cold time-series),
//! and drives the production write/read path over the real adapters. It exercises
//! exactly what cannot be unit-tested: that `HINCRBY` / `PFADD` / `PFCOUNT` /
//! `ZREVRANGE` behave as the domain assumes, that the idempotent flush CTE truly
//! prevents a double-add on redelivery, and that the Scylla counter rollup +
//! range read round-trip.
//!
//! Ingestion is driven through the real `WindowAggregator` → `DeltaFlusher` with
//! fully-formed observations; the wire decode layer is pure and unit-tested, so it
//! is out of scope here.
//!
//! Gated behind `integration-counter` so the default `cargo test -p counter` stays
//! hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p counter --features integration-counter -- --nocapture
//! ```
//!
//! Coverage:
//! - **aggregate** — N folded view observations collapse to the correct live total
//!   (real `HINCRBY`); distinct viewers estimate within HLL error (`PFADD`/`PFCOUNT`).
//! - **idempotency** — re-flushing the same window leaves the durable total
//!   unchanged (the ledger CTE), reported as `already_applied`.
//! - **trending** — relative ranking of entities by score (`ZREVRANGE`).
//! - **timeseries** — window scalars roll into Scylla counter buckets and read back
//!   over a range.
#![cfg(feature = "integration-counter")]

mod counter_it;
