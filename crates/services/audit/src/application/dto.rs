//! Application-layer data-transfer types — the inputs and results of the use
//! cases, independent of both the domain internals and the generated proto.

use chrono::{DateTime, Utc};

use crate::domain::{
    EventCategory, EventId, PartitionKey, RecordHash, SubjectPseudonym, TenantId,
};

/// The durable-commit proof returned when an event is persisted AND chained. For
/// the synchronous `RecordPrivileged` lane this *is* the evidence the caller needs
/// before proceeding; it mirrors what a read returns for the same record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordProof {
    pub event_id: EventId,
    pub partition: PartitionKey,
    pub sequence: u64,
    pub record_hash: RecordHash,
    pub committed_at: DateTime<Utc>,
}

/// The outcome of committing one event to the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Newly persisted + chained, with its proof.
    Committed(RecordProof),
    /// An event with this id was already chained (idempotent replay); the existing
    /// proof is returned. On the async lane this is a benign skip.
    AlreadyRecorded(RecordProof),
}

impl CommitOutcome {
    pub fn proof(&self) -> &RecordProof {
        match self {
            CommitOutcome::Committed(p) | CommitOutcome::AlreadyRecorded(p) => p,
        }
    }

    pub fn is_duplicate(&self) -> bool {
        matches!(self, CommitOutcome::AlreadyRecorded(_))
    }
}

/// A read filter for [`crate::application::QueryHandler`] / exports. All fields are
/// optional; `None` is "not filtered". Authorization (need-to-know) is enforced at
/// the service boundary before a query reaches here.
#[derive(Debug, Clone, Default)]
pub struct LedgerQuery {
    pub subject: Option<SubjectPseudonym>,
    pub tenant: Option<TenantId>,
    pub category: Option<EventCategory>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// Page size after the store clamps it.
    pub limit: usize,
}

/// The result of a ledger integrity verification — a successful *answer*, even
/// when that answer is "tampered". (A tamper finding is not an RPC error; it is
/// the verification doing its job.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityStatus {
    Verified,
    HashMismatch,
    SequenceGap,
    CheckpointDivergence,
}

impl IntegrityStatus {
    pub fn is_verified(self) -> bool {
        matches!(self, IntegrityStatus::Verified)
    }
}

/// The detail of an integrity check over a partition (or the global head set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    pub status: IntegrityStatus,
    /// The highest sequence verified before any divergence (0 for the global
    /// checkpoint check).
    pub verified_through: u64,
    /// The sequence at which divergence was detected, if any.
    pub divergence_at: Option<u64>,
    /// The checkpoint root reconciled against, for the global check.
    pub checkpoint_root: Option<RecordHash>,
}

impl IntegrityReport {
    pub fn verified(through: u64) -> Self {
        Self {
            status: IntegrityStatus::Verified,
            verified_through: through,
            divergence_at: None,
            checkpoint_root: None,
        }
    }
}

/// The manifest of a completed export — a reference to a signed bundle in object
/// storage. The bytes themselves never travel back through the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportManifest {
    pub export_id: String,
    pub record_count: u64,
    pub content_hash: RecordHash,
    pub artifact_ref: String,
    pub generated_at: DateTime<Utc>,
}
