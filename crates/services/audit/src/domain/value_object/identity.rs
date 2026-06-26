use serde::{Deserialize, Serialize};

use crate::error::AuditError;

/// Declares an opaque, non-empty `String` newtype used as a domain identifier.
/// A blank value is a malformed input, surfaced as `AUD-9002 InvalidIdentifier`
/// with the field name. The audit plane never interprets these beyond equality
/// and chain/partition composition — they are pseudonymous references to subjects
/// owned elsewhere (the real identity↔pseudonym mapping lives in `account`).
macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AuditError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(AuditError::InvalidIdentifier($field.to_owned()));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// The deterministic identity of one logical audit event (a UUIDv5 over the
    /// event's content, minted upstream). It is the idempotency key for the hash
    /// chain — a redelivery with the same `EventId` is deduped, so each logical
    /// event is chained exactly once.
    EventId, "event_id"
);

string_id!(
    /// The opaque pseudonym of the data subject an event concerns. Indexed for
    /// subject-scoped query / export / erasure. The audit plane never resolves it
    /// to a real identity — that mapping lives in `account` under its own controls.
    SubjectPseudonym, "subject_pseudonym"
);

string_id!(
    /// A pseudonymous tenant / realm scope. With the event category it derives the
    /// chain [`PartitionKey`]; the subject is carried separately (as an indexed
    /// field), so per-subject erasure never fragments a chain.
    TenantId, "tenant_id"
);

string_id!(
    /// The resolved chain partition an [`crate::domain::AuditRecord`] belongs to.
    /// Partitioning is hybrid (derived from tenant + category): chains stay dense
    /// and parallel, while subject-scoped operations index across them.
    PartitionKey, "partition_key"
);

impl PartitionKey {
    /// Derive the chain partition for an event from its tenant and category — the
    /// hybrid partitioning scheme. Chains are keyed by `(tenant, category)` (dense,
    /// parallel-appendable, with strong ordering within a domain); the subject is
    /// carried as an indexed field on the record, NOT the partition, so erasing a
    /// subject never fragments a chain. Events with no tenant share a `_global`
    /// realm. The category's stable numeric tag is used so the partition name does
    /// not drift if a category is ever renamed.
    pub fn derive(tenant: Option<&TenantId>, category: super::EventCategory) -> Self {
        let realm = tenant.map(TenantId::as_str).unwrap_or("_global");
        // Infallible: the composed string is always non-empty.
        Self::new(format!("{realm}:{}", category.hash_tag()))
            .expect("derived partition key is always non-empty")
    }
}

string_id!(
    /// The pseudonym of the actor that performed an action (a user, admin, or
    /// service principal — never a cleartext identity).
    ActorPseudonym, "actor_pseudonym"
);

string_id!(
    /// A key-vault reference to a per-subject data-encryption key (DEK). It names
    /// the key, never the key material. Destroying the referenced DEK is the
    /// crypto-shred that erases a subject's PII (see [`crate::domain::PiiEnvelope`]).
    SubjectKeyRef, "subject_key_ref"
);

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn accepts_non_empty() {
        assert_eq!(EventId::new("evt-1").unwrap().as_str(), "evt-1");
        assert_eq!(SubjectPseudonym::new("7f3a").unwrap().as_str(), "7f3a");
        assert_eq!(PartitionKey::new("tenant-7:moderation").unwrap().as_str(), "tenant-7:moderation");
    }

    #[test]
    fn rejects_blank_with_field_named_code() {
        let err = SubjectPseudonym::new("   ").unwrap_err();
        assert_eq!(err.error_code(), "AUD-9002");
        assert!(err.to_string().contains("subject_pseudonym"));

        assert_eq!(EventId::new("").unwrap_err().error_code(), "AUD-9002");
    }

    #[test]
    fn partition_derivation_is_hybrid_and_subject_independent() {
        use super::super::EventCategory;
        let tenant = TenantId::new("tenant-7").unwrap();
        // Same (tenant, category) → same partition, regardless of subject.
        let a = PartitionKey::derive(Some(&tenant), EventCategory::Moderation);
        let b = PartitionKey::derive(Some(&tenant), EventCategory::Moderation);
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "tenant-7:3");
        // Different category → different partition.
        assert_ne!(a, PartitionKey::derive(Some(&tenant), EventCategory::Consent));
        // No tenant → the shared global realm.
        assert_eq!(
            PartitionKey::derive(None, EventCategory::Authentication).as_str(),
            "_global:1"
        );
    }
}
