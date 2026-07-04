//! The pure domain layer for the audit & compliance plane — the evidence model
//! and the rules that make it tamper-evident and erasure-reconcilable.
//!
//! Everything here is I/O-free and clock-injected (time arrives as `DateTime<Utc>`
//! parameters): every decision the plane makes about a record is a method that is
//! unit-testable without Postgres, an object store, a key vault, or a wall clock.
//! The generated `audit-api` proto types are deliberately absent; the mapping
//! between these pure types and the wire types lives in the infrastructure tier.
//!
//! Three rules are load-bearing and live in this layer:
//! * **Hash-chain integrity** ([`chain::verify_link`]) — each record links to its
//!   predecessor by `H(prev ‖ payload ‖ seq)` over a monotonic per-partition
//!   sequence, so tampering (hash mismatch) and truncation (sequence gap) are
//!   detectable; [`checkpoint::MerkleCheckpoint`] stitches the partition heads
//!   into one externally-anchorable root that catches operator-level tampering.
//! * **Crypto-shred ⇄ integrity** ([`record::AuditRecord::mark_pii_erased`]) — the
//!   canonical bytes hash the PII *ciphertext*, never plaintext, so destroying the
//!   per-subject key erases the personal data while the record and the whole chain
//!   still verify. The proof survives; the content does not.
//! * **Lawful retention overrides erasure** ([`retention::authorize_erasure`]) —
//!   a subject under an active legal hold (GDPR Art. 17(3)) is not shredded, and a
//!   record is never expired before its retention floor.

pub mod chain;
pub mod checkpoint;
pub mod event;
pub mod record;
pub mod retention;
pub mod value_object;

pub use chain::{ChainHead, ChainLink, verify_link};
pub use checkpoint::MerkleCheckpoint;
pub use event::{Actor, AuditEvent, NewAuditEvent, ResourceRef};
pub use record::AuditRecord;
pub use retention::{LegalHold, RetentionPolicy, authorize_erasure, authorize_expiry};
pub use value_object::{
    ActorPseudonym, ActorType, CanonicalWriter, EventCategory, EventId, LawfulBasis, Outcome,
    PartitionKey, PiiEnvelope, PrivilegedActionType, RecordHash, SubjectKeyRef, SubjectPseudonym,
    TenantId,
};
