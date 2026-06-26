use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::chain::{ChainHead, ChainLink, verify_link};
use crate::domain::event::AuditEvent;
use crate::domain::value_object::{PartitionKey, PiiEnvelope, RecordHash};
use crate::error::AuditError;

/// An [`AuditEvent`] as it sits in the ledger: the immutable event plus the
/// tamper-evidence metadata (its chain link, the partition it belongs to, the
/// ledger record-time) and the crypto-shred state.
///
/// The aggregate's defining behaviour is how it reconciles GDPR erasure with
/// integrity: [`AuditRecord::mark_pii_erased`] flips a flag and nothing else —
/// the event's canonical bytes (which hash the PII *ciphertext*, never plaintext)
/// are untouched, so [`AuditRecord::verify`] still passes after erasure. The
/// proof that the action happened survives; the personal content does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    event: AuditEvent,
    partition: PartitionKey,
    link: ChainLink,
    recorded_at: DateTime<Utc>,
    pii_erased: bool,
}

impl AuditRecord {
    /// Chain `event` onto a partition `head`, returning the new record and the
    /// advanced head. The payload hashed is the event's canonical bytes.
    pub fn append(
        event: AuditEvent,
        partition: PartitionKey,
        head: &ChainHead,
        recorded_at: DateTime<Utc>,
    ) -> (AuditRecord, ChainHead) {
        let payload = event.canonical_bytes();
        let link = head.link_next(&payload);
        let new_head = head.apply(&link);
        let record = AuditRecord {
            event,
            partition,
            link,
            recorded_at,
            pii_erased: false,
        };
        (record, new_head)
    }

    /// Reconstruct a stored record at the ledger boundary (no re-hashing — the
    /// stored link is verified separately via [`AuditRecord::verify`]).
    pub fn from_stored(
        event: AuditEvent,
        partition: PartitionKey,
        link: ChainLink,
        recorded_at: DateTime<Utc>,
        pii_erased: bool,
    ) -> Self {
        Self {
            event,
            partition,
            link,
            recorded_at,
            pii_erased,
        }
    }

    /// Verify this record validly extends `prev_head`, returning the advanced
    /// head. Recomputes over the event's canonical bytes, so it is independent of
    /// the `pii_erased` flag — a crypto-shredded record still verifies.
    pub fn verify(&self, prev_head: &ChainHead) -> Result<ChainHead, AuditError> {
        verify_link(
            &self.partition,
            prev_head,
            &self.event.canonical_bytes(),
            &self.link,
        )
    }

    /// Crypto-shred: mark this record's PII as erased (its per-subject DEK has been
    /// destroyed in the key vault). Idempotent. Returns whether there was a PII
    /// envelope to erase — a record with none is a benign no-op.
    ///
    /// Crucially this does not touch the ciphertext or any hashed field, so the
    /// chain is undisturbed (see the `erasure_preserves_chain_integrity` test).
    pub fn mark_pii_erased(&mut self) -> bool {
        if self.event.has_pii() {
            self.pii_erased = true;
            true
        } else {
            false
        }
    }

    /// Read the PII envelope for decryption upstream. Fails with `AUD-5004` once
    /// the subject has been crypto-shredded — the ciphertext is still here but
    /// permanently undecryptable, which is the *expected* post-erasure state, not
    /// a fault to retry.
    pub fn read_pii(&self) -> Result<&PiiEnvelope, AuditError> {
        if self.pii_erased {
            return Err(AuditError::PiiEnvelopeUndecryptable);
        }
        self.event
            .pii()
            .ok_or(AuditError::PiiEnvelopeUndecryptable)
    }

    pub fn event(&self) -> &AuditEvent {
        &self.event
    }

    pub fn partition(&self) -> &PartitionKey {
        &self.partition
    }

    pub fn sequence(&self) -> u64 {
        self.link.sequence
    }

    pub fn record_hash(&self) -> &RecordHash {
        &self.link.record_hash
    }

    pub fn recorded_at(&self) -> DateTime<Utc> {
        self.recorded_at
    }

    pub fn pii_erased(&self) -> bool {
        self.pii_erased
    }

    /// Test-only: a copy with a different event body but the **original** chain
    /// link — it models a ledger row edited in place after its hash was stored, so
    /// verification recomputes a mismatch. Lives here because it must reach the
    /// record's private fields.
    #[cfg(test)]
    pub(crate) fn tampered_clone(&self, event: AuditEvent) -> AuditRecord {
        AuditRecord {
            event,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;
    use crate::domain::event::fixtures;
    use crate::domain::value_object::EventCategory;

    fn partition() -> PartitionKey {
        PartitionKey::new("tenant-7:moderation").unwrap()
    }

    fn recorded_at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 26, 12, 0, 1).unwrap()
    }

    fn event(id: &str) -> AuditEvent {
        AuditEvent::try_new(fixtures::draft(id, EventCategory::Moderation)).unwrap()
    }

    #[test]
    fn appended_records_form_a_verifiable_chain() {
        let head = ChainHead::genesis();
        let (r1, head) = AuditRecord::append(event("evt-1"), partition(), &head, recorded_at());
        let (r2, _head) = AuditRecord::append(event("evt-2"), partition(), &head, recorded_at());

        let h1 = r1.verify(&ChainHead::genesis()).unwrap();
        assert_eq!(r1.sequence(), 1);
        let h2 = r2.verify(&h1).unwrap();
        assert_eq!(r2.sequence(), 2);
        assert_eq!(h2.hash(), r2.record_hash());
    }

    #[test]
    fn tampering_with_a_stored_record_is_detected() {
        let head = ChainHead::genesis();
        let (good, _) = AuditRecord::append(event("evt-1"), partition(), &head, recorded_at());

        // Rebuild the "stored" record with a different event but the original
        // (now stale) link — i.e. someone edited the row but not the hash.
        let tampered = AuditRecord::from_stored(
            event("evt-CHANGED"),
            good.partition().clone(),
            // reuse the good link by cloning via verify path:
            {
                let payload = good.event().canonical_bytes();
                ChainHead::genesis().link_next(&payload)
            },
            good.recorded_at(),
            false,
        );
        // The link matches the ORIGINAL event; verifying against the changed event
        // body fails.
        let err = tampered.verify(&ChainHead::genesis()).unwrap_err();
        assert_eq!(err.error_code(), "AUD-2001");
    }

    /// The crux: crypto-shred erases the PII yet the hash chain still verifies,
    /// because the ciphertext (not plaintext) is what was hashed and it never moves.
    #[test]
    fn erasure_preserves_chain_integrity() {
        let head = ChainHead::genesis();
        let pii_event = AuditEvent::try_new(fixtures::with_pii("evt-pii")).unwrap();
        let (mut record, _) = AuditRecord::append(pii_event, partition(), &head, recorded_at());

        let before = record.verify(&ChainHead::genesis()).unwrap();
        assert!(record.read_pii().is_ok());

        // Erase the subject.
        assert!(record.mark_pii_erased());

        // The chain STILL verifies, to the identical head.
        let after = record.verify(&ChainHead::genesis()).unwrap();
        assert_eq!(before, after);

        // But the PII is now permanently unreadable.
        assert!(record.pii_erased());
        assert_eq!(record.read_pii().unwrap_err().error_code(), "AUD-5004");
    }

    #[test]
    fn erasing_a_record_without_pii_is_a_noop() {
        let head = ChainHead::genesis();
        let (mut record, _) = AuditRecord::append(event("evt-1"), partition(), &head, recorded_at());
        assert!(!record.mark_pii_erased());
        assert!(!record.pii_erased());
    }
}
