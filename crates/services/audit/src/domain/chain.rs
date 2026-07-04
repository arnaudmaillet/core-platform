use serde::{Deserialize, Serialize};

use crate::domain::value_object::{CanonicalWriter, PartitionKey, RecordHash};
use crate::error::AuditError;

/// The tip of one partition's append-only hash chain: the last sequence assigned
/// and the last record hash. A fresh partition starts at [`ChainHead::genesis`]
/// (sequence 0, the all-zero hash), so the first real record is sequence 1 linked
/// to genesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainHead {
    sequence: u64,
    hash: RecordHash,
}

impl ChainHead {
    pub fn genesis() -> Self {
        Self {
            sequence: 0,
            hash: RecordHash::genesis(),
        }
    }

    pub fn from_parts(sequence: u64, hash: RecordHash) -> Self {
        Self { sequence, hash }
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn hash(&self) -> &RecordHash {
        &self.hash
    }

    /// Compute the link for the next record appended after this head, given the
    /// record's canonical `payload` bytes. The link's hash is
    /// `H(prev_hash ‖ payload ‖ sequence)` — so any change to the payload, the
    /// predecessor, or the position breaks it.
    pub fn link_next(&self, payload: &[u8]) -> ChainLink {
        let sequence = self.sequence + 1;
        let record_hash = link_hash(&self.hash, payload, sequence);
        ChainLink {
            sequence,
            prev_hash: self.hash.clone(),
            record_hash,
        }
    }

    /// Advance the head onto an appended link (assumes the link was produced by or
    /// verified against this head).
    pub fn apply(&self, link: &ChainLink) -> ChainHead {
        ChainHead {
            sequence: link.sequence,
            hash: link.record_hash.clone(),
        }
    }
}

/// One link in a partition chain — the tamper-evidence metadata stored alongside
/// the record. `prev_hash` ties it to its predecessor; `record_hash` is what the
/// next link will chain onto. `sequence` is monotonic per partition, so a hole is
/// a truncation signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    pub sequence: u64,
    pub prev_hash: RecordHash,
    pub record_hash: RecordHash,
}

/// Verify that `link` (with its `payload`) validly extends `prev_head`, returning
/// the advanced head. Three independent checks, each a distinct AUD-2xxx fault:
///
/// * the sequence must be exactly `prev_head.sequence + 1` — a hole means a record
///   was dropped/truncated (`AUD-2002 SequenceGap`);
/// * the link's `prev_hash` must equal the running head hash — broken linkage is a
///   reorder/splice (`AUD-2001 ChainHashMismatch`);
/// * the recomputed `H(prev ‖ payload ‖ seq)` must equal the stored `record_hash`
///   — a mismatch means the payload or metadata was altered (`AUD-2001`).
///
/// None of these are retryable: they are detections, not transient errors. The
/// operator is assumed potentially hostile, so verification is a forensic gate,
/// never a self-heal.
pub fn verify_link(
    partition: &PartitionKey,
    prev_head: &ChainHead,
    payload: &[u8],
    link: &ChainLink,
) -> Result<ChainHead, AuditError> {
    if link.sequence != prev_head.sequence + 1 {
        return Err(AuditError::SequenceGap {
            partition: partition.to_string(),
        });
    }
    if link.prev_hash != *prev_head.hash() {
        return Err(AuditError::ChainHashMismatch {
            sequence: link.sequence,
        });
    }
    if link_hash(&link.prev_hash, payload, link.sequence) != link.record_hash {
        return Err(AuditError::ChainHashMismatch {
            sequence: link.sequence,
        });
    }
    Ok(prev_head.apply(link))
}

/// `H(prev_hash ‖ payload ‖ sequence)` — the one chaining hash, used to both mint
/// and verify a link so the two can never drift.
fn link_hash(prev_hash: &RecordHash, payload: &[u8], sequence: u64) -> RecordHash {
    let mut w = CanonicalWriter::new();
    w.str(prev_hash.as_str()).bytes(payload).u64(sequence);
    w.finish()
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    fn partition() -> PartitionKey {
        PartitionKey::new("tenant-7:moderation").unwrap()
    }

    #[test]
    fn first_link_is_sequence_one_off_genesis() {
        let head = ChainHead::genesis();
        let link = head.link_next(b"payload-1");
        assert_eq!(link.sequence, 1);
        assert_eq!(link.prev_hash, RecordHash::genesis());
    }

    #[test]
    fn a_valid_run_verifies_and_advances() {
        let p = partition();
        let h0 = ChainHead::genesis();
        let l1 = h0.link_next(b"a");
        let h1 = verify_link(&p, &h0, b"a", &l1).unwrap();
        let l2 = h1.link_next(b"b");
        let h2 = verify_link(&p, &h1, b"b", &l2).unwrap();
        assert_eq!(h2.sequence(), 2);
        assert_eq!(h2.hash(), &l2.record_hash);
    }

    #[test]
    fn an_altered_payload_is_a_hash_mismatch() {
        let p = partition();
        let h0 = ChainHead::genesis();
        let l1 = h0.link_next(b"original");
        // Verify the same link against a DIFFERENT payload (the record body was
        // tampered after the hash was stored).
        let err = verify_link(&p, &h0, b"tampered", &l1).unwrap_err();
        assert_eq!(err.error_code(), "AUD-2001");
        assert!(!err.is_retryable());
    }

    #[test]
    fn a_skipped_sequence_is_a_gap() {
        let p = partition();
        let h0 = ChainHead::genesis();
        let l1 = h0.link_next(b"a");
        let h1 = verify_link(&p, &h0, b"a", &l1).unwrap();
        // Forge a link that jumps from seq 1 straight to seq 3.
        let mut l3 = h1.link_next(b"c");
        l3.sequence = 3;
        let err = verify_link(&p, &h1, b"c", &l3).unwrap_err();
        assert_eq!(err.error_code(), "AUD-2002");
        assert!(!err.is_retryable());
    }

    #[test]
    fn broken_linkage_is_a_hash_mismatch() {
        let p = partition();
        let h0 = ChainHead::genesis();
        let l1 = h0.link_next(b"a");
        let h1 = verify_link(&p, &h0, b"a", &l1).unwrap();
        // A link whose prev_hash points at genesis instead of h1 (a splice).
        let mut spliced = h1.link_next(b"b");
        spliced.prev_hash = RecordHash::genesis();
        let err = verify_link(&p, &h1, b"b", &spliced).unwrap_err();
        assert_eq!(err.error_code(), "AUD-2001");
    }
}
