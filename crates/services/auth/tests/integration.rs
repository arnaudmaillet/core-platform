//! Live, container-backed integration suite for the auth service.
//!
//! Auth owns two stores, so this suite boots a real **PostgreSQL** and a real
//! **Redis** container (via the shared `test-support` harness) and drives the
//! production composition root ([`auth::app::App::compose`]) end-to-end through
//! the gRPC handler. The *external* dependencies (the IdP and the `account`
//! service) are stubbed at their port boundaries.
//!
//! The whole binary is gated behind the `integration-auth` feature so the default
//! `cargo test -p auth` stays hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p auth --features integration-auth -- --nocapture
//! ```
//!
//! Coverage:
//! - **lifecycle** — login → introspect → logout invalidates the edge token.
//! - **refresh_reuse** — rotation works; re-presenting a spent token revokes the
//!   whole session generation.
//! - **global_logout** — a generation bump invalidates every session's token.
//! - **persistence_roundtrip** — sessions, refresh-token lineage, and the subject
//!   link are durably written.
#![cfg(feature = "integration-auth")]

mod auth_it;
