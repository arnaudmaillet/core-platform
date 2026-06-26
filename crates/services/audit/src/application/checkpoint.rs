use std::sync::Arc;

use crate::application::port::{CheckpointAnchor, Clock, LedgerStore};
use crate::domain::MerkleCheckpoint;
use crate::error::AuditError;

/// The checkpoint use case — periodically (the worker's anchor loop, Phase 5)
/// snapshots every partition head into one Merkle root and anchors it to the
/// independent witness. That anchored root is what later makes operator-level
/// tampering detectable, since a verifier can reconcile the live chains against a
/// value the database operator never controlled.
pub struct CheckpointHandler {
    ledger: Arc<dyn LedgerStore>,
    anchor: Arc<dyn CheckpointAnchor>,
    clock: Arc<dyn Clock>,
}

impl CheckpointHandler {
    pub fn new(
        ledger: Arc<dyn LedgerStore>,
        anchor: Arc<dyn CheckpointAnchor>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ledger,
            anchor,
            clock,
        }
    }

    /// Compute a checkpoint over the current partition heads and anchor it.
    /// Returns the checkpoint that was anchored.
    pub async fn create_and_anchor(&self) -> Result<MerkleCheckpoint, AuditError> {
        let heads = self.ledger.partition_heads().await?;
        let checkpoint = MerkleCheckpoint::over(&heads, self.clock.now());
        self.anchor.anchor(&checkpoint).await?;
        Ok(checkpoint)
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::event::fixtures;
    use crate::domain::{AuditEvent, EventCategory};

    fn event(id: &str) -> AuditEvent {
        AuditEvent::try_new(fixtures::draft(id, EventCategory::Moderation)).unwrap()
    }

    #[tokio::test]
    async fn checkpoint_is_created_and_becomes_the_latest_anchor() {
        let fx = Fixture::new();
        fx.ingest().ingest(event("evt-1")).await.unwrap();

        let cp = fx.checkpoint().create_and_anchor().await.unwrap();
        assert_eq!(cp.head_count(), 1);

        let latest = fx.anchor.latest_anchored().await.unwrap().unwrap();
        assert_eq!(latest.root(), cp.root());
    }

    #[tokio::test]
    async fn witness_outage_propagates_as_retryable() {
        let fx = Fixture::new();
        fx.ingest().ingest(event("evt-1")).await.unwrap();
        fx.anchor.set_unavailable(true);

        let err = fx.checkpoint().create_and_anchor().await.unwrap_err();
        assert_eq!(err.error_code(), "AUD-2005");
        assert!(err.is_retryable());
    }
}
