use async_trait::async_trait;

use crate::application::dto::LedgerQuery;
use crate::domain::{AuditRecord, ChainHead, EventId, PartitionKey, RecordHash};
use crate::error::AuditError;

/// The append-only, hash-chained canonical ledger — the audit plane's System of
/// Record. The concrete adapter (Phase 4) is Postgres with `UPDATE`/`DELETE`
/// revoked at the role level, so even a compromised application credential can
/// only ever `INSERT`.
///
/// Two semantics are load-bearing:
/// * [`append`](LedgerStore::append) is a **compare-and-append** against the
///   partition's current head. A lost race (another writer advanced the head)
///   is `AUD-2003 ChainHeadConflict` — *retryable*: the caller re-reads the head
///   and re-chains. A genuine store fault is `AUD-4001 LedgerStoreUnavailable`,
///   also retryable, so the `run_consumer` ingest lane never commits past an
///   un-persisted event.
/// * [`lookup`](LedgerStore::lookup) by `event_id` is the idempotency seam — a
///   redelivery is deduped to the already-chained record, so each logical event is
///   chained exactly once.
#[async_trait]
pub trait LedgerStore: Send + Sync + 'static {
    /// The current head of a partition's chain, or [`ChainHead::genesis`] if the
    /// partition has no records yet.
    async fn head(&self, partition: &PartitionKey) -> Result<ChainHead, AuditError>;

    /// Find an already-chained record by its deterministic event id (idempotency
    /// and proof retrieval). `None` means not yet recorded.
    async fn lookup(&self, event_id: &EventId) -> Result<Option<AuditRecord>, AuditError>;

    /// Atomically append `record` iff the partition head is still `expected_head`.
    /// A moved head → `AUD-2003`; an unreachable store → `AUD-4001`.
    async fn append(
        &self,
        record: &AuditRecord,
        expected_head: &ChainHead,
    ) -> Result<(), AuditError>;

    /// Records matching a filter, for the access-controlled query/export reads.
    async fn query(&self, spec: &LedgerQuery) -> Result<Vec<AuditRecord>, AuditError>;

    /// Every record in a partition, in sequence order — the verifier's walk.
    async fn read_partition(
        &self,
        partition: &PartitionKey,
    ) -> Result<Vec<AuditRecord>, AuditError>;

    /// The current `(partition, head_hash)` of every chain — the input to a
    /// global Merkle checkpoint and the global integrity check.
    async fn partition_heads(&self) -> Result<Vec<(PartitionKey, RecordHash)>, AuditError>;
}
