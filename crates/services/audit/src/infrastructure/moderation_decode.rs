//! Audit-side decode of the moderation compliance feed (`moderation.v1.events`)
//! into domain [`AuditEvent`]s. Audit owns this read schema (the consumer-owns
//! -schema tiering rule — it must not depend on the `moderation` crate), so the
//! wire shapes here are lenient structs hand-matched to moderation's published
//! JSON (extra fields ignored, so an additive upstream change never breaks us).
//!
//! Scope (first cut): `decision_recorded` — the dedicated evidence event carrying
//! the authority (`DecisionAuthor`) and the DSA `rationale` — and
//! `enforcement_applied`. Every other moderation event is a benign skip.
//!
//! **This layer is pure and does NOT encrypt.** The `rationale` is PII-bearing, so
//! the caller (the consumer wiring, Phase 3) seals it into a crypto-shreddable
//! [`PiiEnvelope`] via the audit-side cipher and passes it into
//! [`map_decision_recorded`]. Keeping the crypto out here keeps the mapping a
//! total, unit-testable function.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::{
    Actor, ActorPseudonym, ActorType, AuditEvent, EventCategory, EventId, LawfulBasis,
    NewAuditEvent, Outcome, PiiEnvelope, ResourceRef, SubjectPseudonym,
};
use crate::error::AuditError;

pub const TOPIC_MODERATION_EVENTS: &str = "moderation.v1.events";

/// Fixed namespace for deterministic UUIDv5 audit-event ids derived from a
/// moderation event's coordinates — so a redelivery maps to the same id and audit
/// dedupes it. (`b"audit_mod_evt_v5"`.)
const NS_AUDIT_MODERATION: Uuid = Uuid::from_u128(0x6175_6469_745f_6d6f_645f_6576_745f_7635);

const SOURCE: &str = "moderation";

// ── Wire schema (hand-matched to moderation's published JSON) ──────────────────

/// The moderation events audit consumes. `Other` absorbs every event type outside
/// the first-cut scope (case_opened/resolved, enforcement_reversed, appeal_resolved)
/// as a benign skip rather than a poison decode.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModerationEventWire {
    DecisionRecorded(DecisionRecordedWire),
    EnforcementApplied(EnforcementAppliedWire),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecisionRecordedWire {
    pub decision_id: String,
    pub subject: SubjectRefWire,
    pub author: DecisionAuthorWire,
    pub action: String,
    pub category: String,
    pub policy_version: String,
    pub rationale: String,
    #[serde(default)]
    pub reverses: Option<String>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnforcementAppliedWire {
    pub enforcement_id: String,
    pub subject: SubjectRefWire,
    pub action: String,
    pub version: i64,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubjectRefWire {
    #[serde(default)]
    pub entity_type: String,
    #[serde(default)]
    pub entity_id: String,
    /// The affected actor — moderation's `ActorId` (a UUID string); audit's subject
    /// pseudonym. Audit never resolves it; the identity↔real mapping lives in
    /// `account`.
    pub actor_id: String,
    #[serde(default)]
    pub surface: String,
}

/// Externally-tagged, matching moderation's `DecisionAuthor` JSON
/// (`{"Reviewer":"id"}` / `{"Rule":"id"}`).
#[derive(Debug, Clone, Deserialize)]
pub enum DecisionAuthorWire {
    Reviewer(String),
    Rule(String),
}

// ── Mapping ────────────────────────────────────────────────────────────────────

/// Map a `decision_recorded` event to an `AuditEvent`. `sealed_rationale` is the
/// crypto-shreddable envelope the caller produced from the wire's `rationale`.
pub fn map_decision_recorded(
    wire: &DecisionRecordedWire,
    sealed_rationale: PiiEnvelope,
) -> Result<AuditEvent, AuditError> {
    // A "no action" decision is a dismissal / overturn → the content stands
    // (Permitted); any enforcing action is Executed.
    let outcome = if wire.action == "no_action" {
        Outcome::Permitted
    } else {
        Outcome::Executed
    };

    let (actor_type, author_id, author_kind) = match &wire.author {
        DecisionAuthorWire::Reviewer(id) => (ActorType::Admin, id.clone(), "reviewer"),
        DecisionAuthorWire::Rule(id) => (ActorType::System, id.clone(), "rule"),
    };

    let mut attributes = BTreeMap::new();
    attributes.insert("decision_id".to_owned(), wire.decision_id.clone());
    attributes.insert("policy_version".to_owned(), wire.policy_version.clone());
    attributes.insert("category".to_owned(), wire.category.clone());
    attributes.insert("author_kind".to_owned(), author_kind.to_owned());
    if let Some(reverses) = &wire.reverses {
        attributes.insert("reverses".to_owned(), reverses.clone());
    }
    // NB: the rationale is deliberately NOT in attributes — it rides only in the
    // crypto-shreddable PII envelope.

    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&["moderation.decision_recorded", &wire.decision_id]))?,
        category: EventCategory::Moderation,
        subject: Some(SubjectPseudonym::new(wire.subject.actor_id.clone())?),
        tenant: None,
        actor: Actor::new(actor_type, ActorPseudonym::new(author_id)?, String::new()),
        action: format!("moderation.decide.{}", wire.action),
        resource: ResourceRef::new(wire.subject.entity_type.clone(), wire.subject.entity_id.clone()),
        outcome,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: Some(sealed_rationale),
        attributes,
    })
}

/// Map an `enforcement_applied` event to an `AuditEvent`. No rationale → no PII
/// envelope. The applying authority is the system (the decision that authorized it
/// is its own `decision_recorded` event).
pub fn map_enforcement_applied(wire: &EnforcementAppliedWire) -> Result<AuditEvent, AuditError> {
    let mut attributes = BTreeMap::new();
    attributes.insert("enforcement_id".to_owned(), wire.enforcement_id.clone());
    attributes.insert("version".to_owned(), wire.version.to_string());
    if let Some(expires_at) = &wire.expires_at {
        attributes.insert("expires_at".to_owned(), expires_at.to_rfc3339());
    }

    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&[
            "moderation.enforcement_applied",
            &wire.enforcement_id,
            &wire.version.to_string(),
        ]))?,
        category: EventCategory::Moderation,
        subject: Some(SubjectPseudonym::new(wire.subject.actor_id.clone())?),
        tenant: None,
        actor: Actor::new(ActorType::System, ActorPseudonym::new(SOURCE)?, String::new()),
        action: format!("moderation.enforce.{}", wire.action),
        resource: ResourceRef::new(wire.subject.entity_type.clone(), wire.subject.entity_id.clone()),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: None,
        attributes,
    })
}

/// Deterministic UUIDv5 over the event's coordinates — the idempotency key audit
/// dedupes a redelivery on.
fn derive_id(parts: &[&str]) -> String {
    Uuid::new_v5(&NS_AUDIT_MODERATION, parts.join(":").as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SubjectKeyRef;

    fn sealed() -> PiiEnvelope {
        PiiEnvelope::sealed(
            SubjectKeyRef::new("dek:test").unwrap(),
            b"ciphertext".to_vec(),
            b"nonce".to_vec(),
            "AES-256-GCM",
        )
    }

    fn decision_json(author: serde_json::Value, action: &str) -> ModerationEventWire {
        serde_json::from_value(serde_json::json!({
            "type": "decision_recorded",
            "decision_id": "11111111-1111-1111-1111-111111111111",
            "subject": {
                "entity_type": "post",
                "entity_id": "p1",
                "actor_id": "22222222-2222-2222-2222-222222222222",
                "surface": "feed"
            },
            "author": author,
            "action": action,
            "category": "harassment",
            "policy_version": "2026.06.1",
            "rationale": "violates the harassment policy clause 3.2",
            "reverses": null,
            "occurred_at": "2026-06-26T12:00:00Z",
            "correlation_id": "33333333-3333-3333-3333-333333333333"
        }))
        .unwrap()
    }

    #[test]
    fn decision_recorded_maps_reviewer_to_admin_with_sealed_rationale() {
        let wire = match decision_json(serde_json::json!({ "Reviewer": "rev-1" }), "remove_content") {
            ModerationEventWire::DecisionRecorded(d) => d,
            _ => panic!("expected DecisionRecorded"),
        };
        let event = map_decision_recorded(&wire, sealed()).unwrap();

        assert_eq!(event.category(), EventCategory::Moderation);
        assert_eq!(event.actor().actor_type, ActorType::Admin);
        assert_eq!(event.actor().pseudonym.as_str(), "rev-1");
        assert_eq!(event.action(), "moderation.decide.remove_content");
        assert_eq!(event.outcome(), Outcome::Executed);
        assert_eq!(event.subject().unwrap().as_str(), "22222222-2222-2222-2222-222222222222");
        // The rationale rides ONLY in the sealed envelope, never in attributes.
        assert!(event.has_pii());
        assert!(!event.attributes().values().any(|v| v.contains("harassment policy clause")));
        assert_eq!(event.attributes().get("policy_version").unwrap(), "2026.06.1");
        assert_eq!(event.attributes().get("author_kind").unwrap(), "reviewer");
    }

    #[test]
    fn automated_rule_decision_maps_to_system_actor() {
        let wire = match decision_json(serde_json::json!({ "Rule": "screen:hash-match" }), "remove_content") {
            ModerationEventWire::DecisionRecorded(d) => d,
            _ => panic!(),
        };
        let event = map_decision_recorded(&wire, sealed()).unwrap();
        assert_eq!(event.actor().actor_type, ActorType::System);
        assert_eq!(event.actor().pseudonym.as_str(), "screen:hash-match");
    }

    #[test]
    fn dismissal_maps_to_permitted() {
        let wire = match decision_json(serde_json::json!({ "Reviewer": "rev-1" }), "no_action") {
            ModerationEventWire::DecisionRecorded(d) => d,
            _ => panic!(),
        };
        let event = map_decision_recorded(&wire, sealed()).unwrap();
        assert_eq!(event.outcome(), Outcome::Permitted);
        assert_eq!(event.action(), "moderation.decide.no_action");
    }

    #[test]
    fn event_id_is_deterministic_for_the_same_decision() {
        let mk = || match decision_json(serde_json::json!({ "Reviewer": "rev-1" }), "ban") {
            ModerationEventWire::DecisionRecorded(d) => map_decision_recorded(&d, sealed()).unwrap(),
            _ => panic!(),
        };
        assert_eq!(mk().event_id(), mk().event_id());
    }

    #[test]
    fn enforcement_applied_maps_without_pii() {
        let wire: ModerationEventWire = serde_json::from_value(serde_json::json!({
            "type": "enforcement_applied",
            "enforcement_id": "44444444-4444-4444-4444-444444444444",
            "subject": { "entity_type": "account", "entity_id": "acc-9", "actor_id": "55555555-5555-5555-5555-555555555555", "surface": "" },
            "actor_id": "55555555-5555-5555-5555-555555555555",
            "action": "ban",
            "version": 7,
            "expires_at": null,
            "occurred_at": "2026-06-26T12:00:00Z",
            "correlation_id": "66666666-6666-6666-6666-666666666666"
        }))
        .unwrap();
        let wire = match wire {
            ModerationEventWire::EnforcementApplied(e) => e,
            _ => panic!("expected EnforcementApplied"),
        };
        let event = map_enforcement_applied(&wire).unwrap();
        assert_eq!(event.action(), "moderation.enforce.ban");
        assert_eq!(event.outcome(), Outcome::Executed);
        assert!(!event.has_pii());
        assert_eq!(event.actor().actor_type, ActorType::System);
        assert_eq!(event.attributes().get("version").unwrap(), "7");
    }

    #[test]
    fn out_of_scope_events_decode_to_other() {
        for ty in ["case_opened", "case_resolved", "enforcement_reversed", "appeal_resolved"] {
            let wire: ModerationEventWire =
                serde_json::from_value(serde_json::json!({ "type": ty, "actor_id": "x" })).unwrap();
            assert!(matches!(wire, ModerationEventWire::Other), "{ty} should be Other");
        }
    }
}
