use std::sync::Arc;

use crate::application::commit::commit_event;
use crate::application::dto::RecordProof;
use crate::application::port::{Clock, LedgerStore, WormArchive};
use crate::domain::{AuditEvent, PrivilegedActionType};
use crate::error::AuditError;

/// The synchronous, fail-closed write lane. A small, locked set of actions
/// (break-glass access, legal-hold place/release) must be provably recorded
/// *before* they are permitted; this handler returns the durable-commit
/// [`RecordProof`] only once the event is persisted AND chained.
///
/// "Fail-closed" is the whole point: unlike [`crate::application::IngestHandler`],
/// this handler swallows nothing. A ledger/archive fault propagates (the caller
/// must then DENY the action), and the durable-commit deadline that turns a slow
/// store into `AUD-4004` is enforced by the infrastructure timeout wrapping this
/// call (Phase 5). You cannot perform the most dangerous actions precisely when
/// the system cannot prove you did.
///
/// The enrolled set is the [`PrivilegedActionType`] enum itself — the type system
/// enforces the lock, so no runtime widening can sneak in without a contract
/// change.
pub struct RecordPrivilegedHandler {
    ledger: Arc<dyn LedgerStore>,
    archive: Arc<dyn WormArchive>,
    clock: Arc<dyn Clock>,
}

impl RecordPrivilegedHandler {
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

    /// Record a privileged action and return its proof. Idempotent: a replay of
    /// the same event id returns the original proof rather than chaining twice.
    /// `action` names the enrolled operation (informational — the lock is the
    /// type); the durable proof is the contract.
    pub async fn record(
        &self,
        event: AuditEvent,
        action: PrivilegedActionType,
    ) -> Result<RecordProof, AuditError> {
        let _ = action; // enrolled-set lock is enforced by the type; recorded upstream
        let outcome = commit_event(&self.ledger, &self.archive, &self.clock, event).await?;
        Ok(outcome.proof().clone())
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::EventCategory;
    use crate::domain::event::fixtures;

    fn privileged_event(id: &str) -> AuditEvent {
        AuditEvent::try_new(fixtures::draft(id, EventCategory::PrivilegedAction)).unwrap()
    }

    #[tokio::test]
    async fn returns_durable_proof_on_success() {
        let fx = Fixture::new();
        let proof = fx
            .privileged()
            .record(privileged_event("bg-1"), PrivilegedActionType::BreakGlassAccess)
            .await
            .unwrap();
        assert_eq!(proof.sequence, 1);
        assert_eq!(fx.ledger.record_count(), 1);
        assert_eq!(fx.archive.archived_count(), 1);
    }

    #[tokio::test]
    async fn fails_closed_when_the_ledger_is_unavailable() {
        let fx = Fixture::new();
        fx.ledger.set_unavailable(true);
        let err = fx
            .privileged()
            .record(privileged_event("bg-1"), PrivilegedActionType::BreakGlassAccess)
            .await
            .unwrap_err();
        // The caller must DENY the action — nothing was recorded.
        assert_eq!(err.error_code(), "AUD-4001");
        assert_eq!(fx.ledger.record_count(), 0);
    }

    #[tokio::test]
    async fn fails_closed_when_the_archive_is_unavailable() {
        let fx = Fixture::new();
        fx.archive.set_unavailable(true);
        let err = fx
            .privileged()
            .record(privileged_event("bg-1"), PrivilegedActionType::LegalHoldPlace)
            .await
            .unwrap_err();
        assert_eq!(err.error_code(), "AUD-4002");
    }

    #[tokio::test]
    async fn replay_is_idempotent() {
        let fx = Fixture::new();
        let first = fx
            .privileged()
            .record(privileged_event("bg-1"), PrivilegedActionType::BreakGlassAccess)
            .await
            .unwrap();
        let again = fx
            .privileged()
            .record(privileged_event("bg-1"), PrivilegedActionType::BreakGlassAccess)
            .await
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(fx.ledger.record_count(), 1);
    }
}
