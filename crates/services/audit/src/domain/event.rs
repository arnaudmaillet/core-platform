use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{
    ActorPseudonym, ActorType, CanonicalWriter, EventCategory, EventId, LawfulBasis, Outcome,
    PiiEnvelope, RecordHash, SubjectPseudonym, TenantId,
};
use crate::error::AuditError;

/// Who performed an action — a pseudonymous principal. `session_ref` ties the
/// action to an `auth` session without revealing identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub actor_type: ActorType,
    pub pseudonym: ActorPseudonym,
    pub session_ref: String,
}

impl Actor {
    pub fn new(
        actor_type: ActorType,
        pseudonym: ActorPseudonym,
        session_ref: impl Into<String>,
    ) -> Self {
        Self {
            actor_type,
            pseudonym,
            session_ref: session_ref.into(),
        }
    }

    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.u8(self.actor_type.hash_tag())
            .str(self.pseudonym.as_str())
            .str(&self.session_ref);
    }
}

/// A reference to the thing acted upon — type + opaque id, never the content
/// itself. The audit plane stores references, not bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRef {
    pub resource_type: String,
    pub id: String,
}

impl ResourceRef {
    pub fn new(resource_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            resource_type: resource_type.into(),
            id: id.into(),
        }
    }

    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.resource_type).str(&self.id);
    }
}

/// All the fields of an audit event, validated into an immutable [`AuditEvent`]
/// by [`AuditEvent::try_new`].
#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub event_id: EventId,
    pub category: EventCategory,
    /// The data subject the event concerns. `None` for events with no subject
    /// (a pure system action).
    pub subject: Option<SubjectPseudonym>,
    pub tenant: Option<TenantId>,
    pub actor: Actor,
    /// The action verb in a stable vocabulary (e.g. `account.suspend`).
    pub action: String,
    pub resource: ResourceRef,
    pub outcome: Outcome,
    pub lawful_basis: LawfulBasis,
    pub source_service: String,
    pub correlation_id: String,
    /// Event-time (producer clock), distinct from the ledger's record-time.
    pub occurred_at: DateTime<Utc>,
    pub pii: Option<PiiEnvelope>,
    /// Non-PII structured metadata. A `BTreeMap` so canonicalization is
    /// order-independent (sorted by key) and therefore deterministic.
    pub attributes: BTreeMap<String, String>,
}

/// The immutable content of one audit event — the thing that gets hash-chained.
///
/// It carries no raw business content (only a [`ResourceRef`]) and no cleartext
/// PII (only the crypto-shreddable [`PiiEnvelope`]). Construction enforces the
/// content invariants; everything after is read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    event_id: EventId,
    category: EventCategory,
    subject: Option<SubjectPseudonym>,
    tenant: Option<TenantId>,
    actor: Actor,
    action: String,
    resource: ResourceRef,
    outcome: Outcome,
    lawful_basis: LawfulBasis,
    source_service: String,
    correlation_id: String,
    occurred_at: DateTime<Utc>,
    pii: Option<PiiEnvelope>,
    attributes: BTreeMap<String, String>,
}

impl AuditEvent {
    /// Validate and seal an event. Enforces the two content invariants:
    /// * a PII-touching category MUST declare a lawful basis (`AUD-1002`);
    /// * the action verb MUST be present (`AUD-9001`).
    pub fn try_new(input: NewAuditEvent) -> Result<Self, AuditError> {
        if input.category.requires_lawful_basis() && !input.lawful_basis.is_specified() {
            return Err(AuditError::MissingLawfulBasis);
        }
        if input.action.trim().is_empty() {
            return Err(AuditError::DomainViolation {
                field: "action".to_owned(),
                message: "an audit event must name the action performed".to_owned(),
            });
        }
        Ok(Self {
            event_id: input.event_id,
            category: input.category,
            subject: input.subject,
            tenant: input.tenant,
            actor: input.actor,
            action: input.action,
            resource: input.resource,
            outcome: input.outcome,
            lawful_basis: input.lawful_basis,
            source_service: input.source_service,
            correlation_id: input.correlation_id,
            occurred_at: input.occurred_at,
            pii: input.pii,
            attributes: input.attributes,
        })
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn category(&self) -> EventCategory {
        self.category
    }

    pub fn subject(&self) -> Option<&SubjectPseudonym> {
        self.subject.as_ref()
    }

    pub fn tenant(&self) -> Option<&TenantId> {
        self.tenant.as_ref()
    }

    pub fn lawful_basis(&self) -> LawfulBasis {
        self.lawful_basis
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    pub fn has_pii(&self) -> bool {
        self.pii.is_some()
    }

    pub fn pii(&self) -> Option<&PiiEnvelope> {
        self.pii.as_ref()
    }

    /// The deterministic canonical bytes of the event — the input the hash chain
    /// digests. Field order is fixed and every field is length-prefixed by
    /// [`CanonicalWriter`]; optional fields write a presence byte then their value
    /// so a present-empty and an absent field never collide. The PII contributes
    /// its **ciphertext**, never plaintext — that is what lets crypto-shred erase
    /// a subject without disturbing the chain.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut w = CanonicalWriter::new();
        w.str(self.event_id.as_str())
            .u8(self.category.hash_tag());

        match &self.subject {
            Some(s) => {
                w.u8(1).str(s.as_str());
            }
            None => {
                w.u8(0);
            }
        }
        match &self.tenant {
            Some(t) => {
                w.u8(1).str(t.as_str());
            }
            None => {
                w.u8(0);
            }
        }

        self.actor.write_canonical(&mut w);
        w.str(&self.action);
        self.resource.write_canonical(&mut w);
        w.u8(self.outcome.hash_tag())
            .u8(self.lawful_basis.hash_tag())
            .str(&self.source_service)
            .str(&self.correlation_id)
            .i64(self.occurred_at.timestamp_millis());

        match &self.pii {
            Some(p) => {
                w.u8(1);
                p.write_canonical(&mut w);
            }
            None => {
                w.u8(0);
            }
        }

        // BTreeMap iterates in sorted key order → deterministic regardless of
        // insertion order. Count-prefixed so the map's boundary is unambiguous.
        w.u64(self.attributes.len() as u64);
        for (k, v) in &self.attributes {
            w.str(k).str(v);
        }

        w.as_bytes().to_vec()
    }

    /// The content hash of this event in isolation (not yet chained). The chain
    /// combines this with the predecessor hash and sequence — see
    /// [`crate::domain::chain`].
    pub fn content_hash(&self) -> RecordHash {
        RecordHash::digest(&self.canonical_bytes())
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use chrono::TimeZone;

    use super::*;
    use crate::domain::value_object::{ActorPseudonym, EventId, SubjectKeyRef};

    /// A minimal valid event for tests; callers tweak fields on the returned
    /// `NewAuditEvent` before sealing.
    pub fn draft(event_id: &str, category: EventCategory) -> NewAuditEvent {
        NewAuditEvent {
            event_id: EventId::new(event_id).unwrap(),
            category,
            subject: Some(SubjectPseudonym::new("7f3a").unwrap()),
            tenant: Some(TenantId::new("tenant-7").unwrap()),
            actor: Actor::new(
                ActorType::Admin,
                ActorPseudonym::new("adm-1").unwrap(),
                "sess-1",
            ),
            action: "account.suspend".to_owned(),
            resource: ResourceRef::new("account", "acc-9"),
            outcome: Outcome::Executed,
            lawful_basis: LawfulBasis::LegalObligation,
            source_service: "moderation".to_owned(),
            correlation_id: "trace-1".to_owned(),
            occurred_at: Utc.with_ymd_and_hms(2026, 6, 26, 12, 0, 0).unwrap(),
            pii: None,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_pii(event_id: &str) -> NewAuditEvent {
        let mut d = draft(event_id, EventCategory::DataAccess);
        d.pii = Some(PiiEnvelope::sealed(
            SubjectKeyRef::new("dek:7f3a").unwrap(),
            b"ciphertext".to_vec(),
            b"nonce".to_vec(),
            "AES-256-GCM",
        ));
        d
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::domain::value_object::EventCategory;

    #[test]
    fn pii_category_without_lawful_basis_is_rejected() {
        let mut d = fixtures::draft("evt-1", EventCategory::Consent);
        d.lawful_basis = LawfulBasis::Unspecified;
        let err = AuditEvent::try_new(d).unwrap_err();
        assert_eq!(err.error_code(), "AUD-1002");
    }

    #[test]
    fn system_category_without_lawful_basis_is_allowed() {
        let mut d = fixtures::draft("evt-1", EventCategory::Authentication);
        d.lawful_basis = LawfulBasis::Unspecified;
        assert!(AuditEvent::try_new(d).is_ok());
    }

    #[test]
    fn empty_action_is_a_domain_violation() {
        let mut d = fixtures::draft("evt-1", EventCategory::Authentication);
        d.action = "   ".to_owned();
        let err = AuditEvent::try_new(d).unwrap_err();
        assert_eq!(err.error_code(), "AUD-9001");
    }

    #[test]
    fn canonical_bytes_are_stable_for_equal_events() {
        let a = AuditEvent::try_new(fixtures::draft("evt-1", EventCategory::Moderation)).unwrap();
        let b = AuditEvent::try_new(fixtures::draft("evt-1", EventCategory::Moderation)).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn attribute_order_does_not_change_the_hash() {
        let mut d1 = fixtures::draft("evt-1", EventCategory::Moderation);
        d1.attributes.insert("z".into(), "1".into());
        d1.attributes.insert("a".into(), "2".into());
        let mut d2 = fixtures::draft("evt-1", EventCategory::Moderation);
        d2.attributes.insert("a".into(), "2".into());
        d2.attributes.insert("z".into(), "1".into());
        let a = AuditEvent::try_new(d1).unwrap();
        let b = AuditEvent::try_new(d2).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn different_content_changes_the_hash() {
        let a = AuditEvent::try_new(fixtures::draft("evt-1", EventCategory::Moderation)).unwrap();
        let mut d = fixtures::draft("evt-1", EventCategory::Moderation);
        d.action = "account.unsuspend".to_owned();
        let b = AuditEvent::try_new(d).unwrap();
        assert_ne!(a.content_hash(), b.content_hash());
    }
}
