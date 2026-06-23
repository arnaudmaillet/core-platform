//! Live, container-backed integration suite for the account microservice.
//!
//! Account is the platform's only relational service, so — unlike every other
//! suite — this one boots a real **PostgreSQL** container (no replication
//! rewrite) and applies the `.sql` migrations through the shared runner.
//!
//! The whole binary is gated behind the `integration-account` feature so the
//! default `cargo test -p account` stays hermetic and Docker-free. Run the live
//! suite explicitly:
//!
//! ```text
//! cargo test -p account --features integration-account -- --nocapture
//! ```
//!
//! It drives the service through the production composition root
//! ([`account::app::App`]):
//!
//! - **uniqueness race** — concurrent creates of the same email resolve to
//!   exactly one winner via the Postgres unique index; the rest are rejected.
//! - **persistence round-trip** — a create is durably written and reflected by
//!   the identity projection.
//!
//! All cross-component synchronisation polls observable state with a deadline
//! (`await_until`); there are no fixed sleeps.
#![cfg(feature = "integration-account")]

mod account_it;
