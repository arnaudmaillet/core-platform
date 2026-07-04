use std::sync::Arc;

use crate::application::commit::commit_event;
use crate::application::dto::CommitOutcome;
use crate::application::port::{Clock, EventSource, LedgerStore, WormArchive};
use crate::domain::AuditEvent;
use crate::error::AuditError;

/// The asynchronous ingestion use case — the fail-open, zero-loss lane that
/// carries ~99% of audit traffic. It chains a decoded event onto the ledger and
/// archives it; a redelivery is deduped to a benign skip.
///
/// Fail-open is about the *producer*, not durability: a producer never blocks on
/// audit (Kafka is the buffer), but the worker still commits its offset only after
/// the event is durably persisted AND chained. Any store fault propagates as a
/// retryable error so the `run_consumer` runtime retries without advancing the
/// offset — nothing is ever lost.
pub struct IngestHandler {
    ledger: Arc<dyn LedgerStore>,
    archive: Arc<dyn WormArchive>,
    clock: Arc<dyn Clock>,
}

impl IngestHandler {
    pub fn new(
        ledger: Arc<dyn LedgerStore>,
        archive: Arc<dyn WormArchive>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ledger,
            archive,
            clock,
        }
    }

    pub async fn ingest(&self, event: AuditEvent) -> Result<CommitOutcome, AuditError> {
        commit_event(&self.ledger, &self.archive, &self.clock, event).await
    }
}

/// Drive ingestion: pull events from `source` and chain each until drained. This
/// is the testable core of the worker's ingest lane; the production binary
/// (Phase 5) wraps the Kafka `run_consumer` runtime (manual commit, backoff/jitter,
/// DLQ) around the same [`IngestHandler::ingest`] call.
pub async fn run_ingest(
    source: &dyn EventSource,
    handler: &IngestHandler,
) -> Result<(), AuditError> {
    while let Some(event) = source.next_event().await? {
        handler.ingest(event).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::EventCategory;
    use crate::domain::event::fixtures;

    fn event(id: &str) -> AuditEvent {
        AuditEvent::try_new(fixtures::draft(id, EventCategory::Moderation)).unwrap()
    }

    #[tokio::test]
    async fn ingest_chains_and_archives() {
        let fx = Fixture::new();
        let out = fx.ingest().ingest(event("evt-1")).await.unwrap();

        assert!(!out.is_duplicate());
        assert_eq!(out.proof().sequence, 1);
        assert_eq!(fx.ledger.record_count(), 1);
        assert_eq!(fx.archive.archived_count(), 1);
    }

    #[tokio::test]
    async fn sequences_advance_within_a_partition() {
        let fx = Fixture::new();
        let p1 = fx.ingest().ingest(event("evt-1")).await.unwrap();
        let p2 = fx.ingest().ingest(event("evt-2")).await.unwrap();
        assert_eq!(p1.proof().sequence, 1);
        assert_eq!(p2.proof().sequence, 2);
        assert_eq!(p1.proof().partition, p2.proof().partition);
    }

    #[tokio::test]
    async fn redelivery_is_deduped_to_the_same_proof() {
        let fx = Fixture::new();
        let first = fx.ingest().ingest(event("evt-1")).await.unwrap();
        let again = fx.ingest().ingest(event("evt-1")).await.unwrap();

        assert!(!first.is_duplicate());
        assert!(again.is_duplicate());
        assert_eq!(first.proof(), again.proof());
        assert_eq!(fx.ledger.record_count(), 1); // chained exactly once
    }

    #[tokio::test]
    async fn a_lost_append_race_is_retried() {
        let fx = Fixture::new();
        fx.ledger.inject_conflicts(3); // first 3 appends lose the race
        let out = fx.ingest().ingest(event("evt-1")).await.unwrap();
        assert_eq!(out.proof().sequence, 1);
        assert_eq!(fx.ledger.record_count(), 1);
    }

    #[tokio::test]
    async fn ledger_outage_propagates_as_retryable_no_loss() {
        use error::AppError;
        let fx = Fixture::new();
        fx.ledger.set_unavailable(true);
        let err = fx.ingest().ingest(event("evt-1")).await.unwrap_err();
        assert_eq!(err.error_code(), "AUD-4001");
        assert!(err.is_retryable());
        assert_eq!(fx.archive.archived_count(), 0);
    }

    #[tokio::test]
    async fn run_ingest_drains_the_source() {
        let fx = Fixture::new();
        fx.source.push(event("evt-1"));
        fx.source.push(event("evt-2"));
        fx.source.push(event("evt-1")); // duplicate → benign skip, still consumed

        run_ingest(fx.source.as_ref(), &fx.ingest()).await.unwrap();

        assert!(fx.source.is_drained());
        assert_eq!(fx.ledger.record_count(), 2);
    }
}
