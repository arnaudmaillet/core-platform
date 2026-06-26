use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{CanonicalWriter, PartitionKey, RecordHash};
use crate::error::AuditError;

/// A signed checkpoint: a single Merkle root computed over every partition's chain
/// head at a point in time. Periodically signed (in a separate KMS trust domain)
/// and anchored to an independent external witness, it is what makes tampering
/// detectable even by an operator who controls the database — the per-partition
/// chains keep the write path parallel, and this root stitches their heads into
/// one comparable value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleCheckpoint {
    root: RecordHash,
    head_count: u64,
    created_at: DateTime<Utc>,
}

impl MerkleCheckpoint {
    /// Compute a checkpoint over the given partition heads. The heads are sorted
    /// by partition key first, so the root is independent of iteration order
    /// (chains live in a map); the count is folded in so a missing partition
    /// cannot be masked by a collision.
    pub fn over(heads: &[(PartitionKey, RecordHash)], created_at: DateTime<Utc>) -> Self {
        Self {
            root: compute_root(heads),
            head_count: heads.len() as u64,
            created_at,
        }
    }

    pub fn root(&self) -> &RecordHash {
        &self.root
    }

    pub fn head_count(&self) -> u64 {
        self.head_count
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Recompute the root over `heads` and confirm it matches this checkpoint.
    /// A mismatch means the live chains diverge from what was anchored — an
    /// operator-level tampering signal (`AUD-2004`), never a self-heal.
    pub fn verify_against(&self, heads: &[(PartitionKey, RecordHash)]) -> Result<(), AuditError> {
        let recomputed = compute_root(heads);
        if recomputed != self.root || heads.len() as u64 != self.head_count {
            return Err(AuditError::CheckpointVerificationFailed);
        }
        Ok(())
    }
}

/// Deterministic root: sort heads by partition, then fold (partition, hash) pairs
/// — count-prefixed and length-prefixed — into a single SHA-256.
fn compute_root(heads: &[(PartitionKey, RecordHash)]) -> RecordHash {
    let mut sorted: Vec<&(PartitionKey, RecordHash)> = heads.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut w = CanonicalWriter::new();
    w.u64(sorted.len() as u64);
    for (partition, hash) in sorted {
        w.str(partition.as_str()).str(hash.as_str());
    }
    w.finish()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 26, 12, 0, 0).unwrap()
    }

    fn head(p: &str, h: &str) -> (PartitionKey, RecordHash) {
        (PartitionKey::new(p).unwrap(), RecordHash::digest(h.as_bytes()))
    }

    #[test]
    fn root_is_order_independent() {
        let a = MerkleCheckpoint::over(&[head("p1", "x"), head("p2", "y")], now());
        let b = MerkleCheckpoint::over(&[head("p2", "y"), head("p1", "x")], now());
        assert_eq!(a.root(), b.root());
    }

    #[test]
    fn verify_passes_for_unchanged_heads() {
        let heads = vec![head("p1", "x"), head("p2", "y")];
        let cp = MerkleCheckpoint::over(&heads, now());
        assert!(cp.verify_against(&heads).is_ok());
    }

    #[test]
    fn a_moved_head_diverges_from_the_checkpoint() {
        let cp = MerkleCheckpoint::over(&[head("p1", "x"), head("p2", "y")], now());
        // p2 advanced (or was rewritten) after the checkpoint was anchored.
        let err = cp
            .verify_against(&[head("p1", "x"), head("p2", "TAMPERED")])
            .unwrap_err();
        assert_eq!(err.error_code(), "AUD-2004");
        assert!(!err.is_retryable());
    }

    #[test]
    fn a_dropped_partition_is_detected() {
        let cp = MerkleCheckpoint::over(&[head("p1", "x"), head("p2", "y")], now());
        let err = cp.verify_against(&[head("p1", "x")]).unwrap_err();
        assert_eq!(err.error_code(), "AUD-2004");
    }
}
