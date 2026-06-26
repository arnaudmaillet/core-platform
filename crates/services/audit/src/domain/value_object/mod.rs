//! Pure value objects for the audit evidence model. No I/O, no clock reads (time
//! is injected as `DateTime<Utc>` parameters), no transport or store awareness,
//! and no dependency on the generated `audit-api` types (the proto mapping lives
//! in the infrastructure tier).

pub mod classification;
pub mod hash;
pub mod identity;
pub mod pii;

pub use classification::{ActorType, EventCategory, LawfulBasis, Outcome, PrivilegedActionType};
pub use hash::{CanonicalWriter, RecordHash};
pub use identity::{
    ActorPseudonym, EventId, PartitionKey, SubjectKeyRef, SubjectPseudonym, TenantId,
};
pub use pii::PiiEnvelope;
