//! The shared commit routine behind both write lanes: idempotent,
//! compare-and-append onto the partition chain, then archive. The async ingest
//! lane folds its `AlreadyRecorded` outcome into a benign skip; the synchronous
//! `RecordPrivileged` lane returns the proof (and fails closed on any store
//! fault). Keeping it in one place means the two lanes can never chain
//! differently.

use std::sync::Arc;

use crate::application::dto::{CommitOutcome, RecordProof};
use crate::application::port::{Clock, LedgerStore, WormArchive};
use crate::domain::{AuditEvent, AuditRecord, PartitionKey};
use crate::error::AuditError;

/// How many times a lost compare-and-append race (`AUD-2003`) is retried before
/// giving up. A conflict means a concurrent writer advanced the same partition
/// head; re-reading and re-chaining resolves it. Exhaustion returns the retryable
/// error so the `run_consumer` driver can try again without committing the offset.
const MAX_APPEND_RETRIES: usize = 8;

/// Idempotently commit one event: dedupe by id, compare-and-append onto the
/// derived partition chain (retrying a lost race), then archive.
pub(crate) async fn commit_event(
    ledger: &Arc<dyn LedgerStore>,
    archive: &Arc<dyn WormArchive>,
    clock: &Arc<dyn Clock>,
    event: AuditEvent,
) -> Result<CommitOutcome, AuditError> {
    // Idempotency: a redelivery of an already-chained event returns the existing
    // proof rather than chaining it twice.
    if let Some(existing) = ledger.lookup(event.event_id()).await? {
        return Ok(CommitOutcome::AlreadyRecorded(proof_of(&existing)));
    }

    let partition = PartitionKey::derive(event.tenant(), event.category());

    let mut attempt = 0;
    loop {
        let head = ledger.head(&partition).await?;
        let recorded_at = clock.now();
        let (record, _new_head) =
            AuditRecord::append(event.clone(), partition.clone(), &head, recorded_at);

        match ledger.append(&record, &head).await {
            Ok(()) => {
                // The ledger is the synchronous source of truth; the archive is the
                // durable backstop written immediately after.
                archive.archive(&record).await?;
                return Ok(CommitOutcome::Committed(proof_of(&record)));
            }
            Err(AuditError::ChainHeadConflict { .. }) if attempt + 1 < MAX_APPEND_RETRIES => {
                attempt += 1;
                continue;
            }
            Err(other) => return Err(other),
        }
    }
}

/// Build the durable-commit proof from a chained record.
pub(crate) fn proof_of(record: &AuditRecord) -> RecordProof {
    RecordProof {
        event_id: record.event().event_id().clone(),
        partition: record.partition().clone(),
        sequence: record.sequence(),
        record_hash: record.record_hash().clone(),
        committed_at: record.recorded_at(),
    }
}
