//! Live, container-backed integration suite for the search service.
//!
//! Search has exactly one backing store, so this suite boots a real single-node
//! **OpenSearch** via the shared `test-support` harness (with the same analyzers,
//! external-versioning, and `delete_by_query` semantics production runs) and drives
//! the production composition root ([`search::app::App::compose`]) end-to-end: the
//! ingestion path through [`search::application::command::ProjectionHandler`] over
//! the real [`search::infrastructure::index::OpenSearchIndex`], and the query path
//! through the gRPC handler.
//!
//! The thin-event decode + gRPC hydration are exercised by unit tests; here the
//! harness drives the projection with fully-formed `SourceEvent`s, so the suite
//! focuses on exactly what needs a live engine: the two version guards, moderation
//! visibility, GDPR purge, and typo-tolerant federated ranking.
//!
//! Gated behind `integration-search` so the default `cargo test -p search` stays
//! hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p search --features integration-search -- --nocapture
//! ```
//!
//! Coverage:
//! - **versioning** — an out-of-order (stale) edit is rejected by external
//!   versioning; a newer edit fully re-projects the document.
//! - **moderation** — a hide removes a document from results and a reversal
//!   restores it; a hide survives a later content edit (the two-guard invariant).
//! - **query** — typo-tolerant federated match across kinds, entity-type filtering,
//!   block-author exclusion, and deep GDPR purge by author.
#![cfg(feature = "integration-search")]

mod search_it;
