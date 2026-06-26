use std::sync::Arc;

use crate::application::dto::{IntegrityReport, IntegrityStatus};
use crate::application::port::{CheckpointAnchor, LedgerStore};
use crate::domain::{ChainHead, PartitionKey};
use crate::error::AuditError;

/// The integrity verification use case — the on-demand tamper-evidence proof.
///
/// A tamper or truncation finding is a *successful answer*, not an RPC failure:
/// these handlers translate the domain's `AUD-2xxx` chain faults into an
/// [`IntegrityReport`] the caller (and the regulator) can read, rather than
/// propagating them. A genuine *availability* fault (the store is down) still
/// propagates, because that is "we couldn't check", not "we checked and it's bad".
pub struct VerifyHandler {
    ledger: Arc<dyn LedgerStore>,
    anchor: Arc<dyn CheckpointAnchor>,
}

impl VerifyHandler {
    pub fn new(ledger: Arc<dyn LedgerStore>, anchor: Arc<dyn CheckpointAnchor>) -> Self {
        Self { ledger, anchor }
    }

    /// Walk a partition's chain from genesis, recomputing every link. Stops at the
    /// first divergence and reports it; otherwise reports `Verified` through the
    /// head.
    pub async fn verify_partition(
        &self,
        partition: &PartitionKey,
    ) -> Result<IntegrityReport, AuditError> {
        let records = self.ledger.read_partition(partition).await?;
        let mut head = ChainHead::genesis();

        for record in &records {
            match record.verify(&head) {
                Ok(next) => head = next,
                Err(AuditError::ChainHashMismatch { sequence }) => {
                    return Ok(report(
                        IntegrityStatus::HashMismatch,
                        head.sequence(),
                        Some(sequence),
                    ));
                }
                Err(AuditError::SequenceGap { .. }) => {
                    return Ok(report(
                        IntegrityStatus::SequenceGap,
                        head.sequence(),
                        Some(record.sequence()),
                    ));
                }
                // Anything else (e.g. an availability fault) is "couldn't verify".
                Err(other) => return Err(other),
            }
        }

        Ok(IntegrityReport::verified(head.sequence()))
    }

    /// Reconcile the live partition heads against the latest externally-anchored
    /// Merkle checkpoint — the global, operator-level tamper check. With nothing
    /// anchored yet, reports `Verified` (there is nothing to disagree with).
    pub async fn verify_global(&self) -> Result<IntegrityReport, AuditError> {
        let heads = self.ledger.partition_heads().await?;
        let Some(checkpoint) = self.anchor.latest_anchored().await? else {
            return Ok(IntegrityReport::verified(0));
        };

        match checkpoint.verify_against(&heads) {
            Ok(()) => Ok(IntegrityReport {
                status: IntegrityStatus::Verified,
                verified_through: 0,
                divergence_at: None,
                checkpoint_root: Some(checkpoint.root().clone()),
            }),
            Err(AuditError::CheckpointVerificationFailed) => Ok(IntegrityReport {
                status: IntegrityStatus::CheckpointDivergence,
                verified_through: 0,
                divergence_at: None,
                checkpoint_root: Some(checkpoint.root().clone()),
            }),
            Err(other) => Err(other),
        }
    }
}

fn report(status: IntegrityStatus, verified_through: u64, divergence_at: Option<u64>) -> IntegrityReport {
    IntegrityReport {
        status,
        verified_through,
        divergence_at,
        checkpoint_root: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::event::fixtures;
    use crate::domain::{AuditEvent, EventCategory, MerkleCheckpoint};

    fn event(id: &str) -> AuditEvent {
        AuditEvent::try_new(fixtures::draft(id, EventCategory::Moderation)).unwrap()
    }

    async fn seed_two(fx: &Fixture) -> PartitionKey {
        let p1 = fx.ingest().ingest(event("evt-1")).await.unwrap();
        fx.ingest().ingest(event("evt-2")).await.unwrap();
        p1.proof().partition.clone()
    }

    #[tokio::test]
    async fn a_clean_chain_verifies() {
        let fx = Fixture::new();
        let partition = seed_two(&fx).await;
        let report = fx.verify().verify_partition(&partition).await.unwrap();
        assert_eq!(report.status, IntegrityStatus::Verified);
        assert_eq!(report.verified_through, 2);
    }

    #[tokio::test]
    async fn a_tampered_record_reports_hash_mismatch() {
        let fx = Fixture::new();
        let partition = seed_two(&fx).await;
        // Mutate a stored record's body in place (a rogue UPDATE).
        fx.ledger.corrupt_payload_at(&partition, 1);

        let report = fx.verify().verify_partition(&partition).await.unwrap();
        assert_eq!(report.status, IntegrityStatus::HashMismatch);
        assert_eq!(report.divergence_at, Some(1));
    }

    #[tokio::test]
    async fn a_dropped_record_reports_a_sequence_gap() {
        let fx = Fixture::new();
        let partition = seed_two(&fx).await;
        fx.ingest().ingest(event("evt-3")).await.unwrap();
        // Delete the middle record (a truncation/splice).
        fx.ledger.delete_at(&partition, 2);

        let report = fx.verify().verify_partition(&partition).await.unwrap();
        assert_eq!(report.status, IntegrityStatus::SequenceGap);
        assert_eq!(report.divergence_at, Some(3));
    }

    #[tokio::test]
    async fn global_check_passes_against_a_matching_anchor() {
        let fx = Fixture::new();
        seed_two(&fx).await;
        let heads = fx.ledger.partition_heads().await.unwrap();
        fx.anchor
            .anchor(&MerkleCheckpoint::over(&heads, fx.now()))
            .await
            .unwrap();

        let report = fx.verify().verify_global().await.unwrap();
        assert_eq!(report.status, IntegrityStatus::Verified);
        assert!(report.checkpoint_root.is_some());
    }

    /// The case the global checkpoint exists for: a TAIL truncation. Per-partition
    /// verification still passes (the remaining prefix is a valid chain), but the
    /// head has regressed below the anchored root — only the externally-anchored
    /// checkpoint catches it.
    #[tokio::test]
    async fn global_check_detects_tail_truncation_after_anchoring() {
        let fx = Fixture::new();
        let partition = seed_two(&fx).await;
        let heads = fx.ledger.partition_heads().await.unwrap();
        fx.anchor
            .anchor(&MerkleCheckpoint::over(&heads, fx.now()))
            .await
            .unwrap();

        // Drop the head record after the checkpoint was anchored.
        fx.ledger.delete_at(&partition, 2);

        // The surviving prefix still verifies on its own...
        let partition_report = fx.verify().verify_partition(&partition).await.unwrap();
        assert_eq!(partition_report.status, IntegrityStatus::Verified);
        // ...but the global checkpoint catches the regressed head.
        let global = fx.verify().verify_global().await.unwrap();
        assert_eq!(global.status, IntegrityStatus::CheckpointDivergence);
    }

    #[tokio::test]
    async fn global_check_with_nothing_anchored_is_verified() {
        let fx = Fixture::new();
        seed_two(&fx).await;
        let report = fx.verify().verify_global().await.unwrap();
        assert_eq!(report.status, IntegrityStatus::Verified);
    }
}
