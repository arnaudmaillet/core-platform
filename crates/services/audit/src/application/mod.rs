//! The audit application layer — use-case orchestration over the domain and the
//! ports. Every external dependency is an `async_trait` port in [`port`], injected
//! as `Arc<dyn …>` at the composition root; in-memory fakes back the unit tests,
//! and the live adapters are Phase 4.
//!
//! ## Handlers
//! The two write lanes share one commit routine ([`commit`]) so they can never
//! chain differently — only their posture differs:
//! * [`IngestHandler`] + [`run_ingest`] — the async, fail-open, zero-loss lane
//!   (~99% of traffic): dedupe → compare-and-append → archive. A redelivery is a
//!   benign skip; a store fault is retryable so the offset never advances past an
//!   un-persisted event.
//! * [`RecordPrivilegedHandler`] — the synchronous, **fail-closed** lane for the
//!   locked privileged set: returns the durable-commit proof, and swallows
//!   nothing (a fault means the caller must deny the action).
//! * [`CryptoShredHandler`] — GDPR erasure as key destruction, gated by the
//!   legal-hold override; never touches the ledger.
//! * [`VerifyHandler`] — turns the domain's chain faults into an
//!   [`IntegrityReport`] (a tamper finding is an answer, not an error).
//! * [`CheckpointHandler`] — snapshots the partition heads into an anchored
//!   Merkle root.
//! * [`QueryHandler`] / [`ExportHandler`] — the access-controlled reads (authz +
//!   read-auditing wire at the service boundary in Phase 5).

pub mod checkpoint;
pub mod commit;
pub mod dto;
pub mod ingest;
pub mod port;
pub mod privileged;
pub mod query;
pub mod shred;
pub mod verify;

#[cfg(test)]
pub mod fakes;

pub use checkpoint::CheckpointHandler;
pub use dto::{
    CommitOutcome, ExportManifest, IntegrityReport, IntegrityStatus, LedgerQuery, RecordProof,
};
pub use ingest::{IngestHandler, run_ingest};
pub use port::{
    CheckpointAnchor, Clock, EventSource, KeyVault, LedgerStore, WormArchive,
};
pub use privileged::RecordPrivilegedHandler;
pub use query::{ExportHandler, QueryHandler};
pub use shred::CryptoShredHandler;
pub use verify::VerifyHandler;
