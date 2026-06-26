//! Audit-side decode of the account feed (`account.v1.events`) into domain
//! [`AuditEvent`]s. Audit owns this read schema (no dependency on the `account`
//! crate); the wire shapes are lenient structs hand-matched to account's published
//! JSON.
//!
//! Scope (first cut): the consent/PII-lifecycle core — `account_created` and
//! `email_changed` (both PII-bearing → the personal data is sealed into a
//! crypto-shreddable envelope), plus the two GDPR events `gdpr_deletion_requested`
//! (Art. 17 — also drives audit's crypto-shred, see the consumer) and
//! `gdpr_data_export_requested` (Art. 15/20). Every other account event is a benign
//! skip.
//!
//! Like the moderation feed, the PII maps take an already-sealed [`PiiEnvelope`]
//! (the consumer seals via the audit cipher) so this layer stays pure.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::{
    Actor, ActorPseudonym, ActorType, AuditEvent, EventCategory, EventId, LawfulBasis,
    NewAuditEvent, Outcome, PiiEnvelope, ResourceRef, SubjectPseudonym,
};
use crate::error::AuditError;

pub const TOPIC_ACCOUNT_EVENTS: &str = "account.v1.events";

/// Namespace for deterministic UUIDv5 audit-event ids. (`b"audit_acct_evt_v5"`.)
const NS_AUDIT_ACCOUNT: Uuid = Uuid::from_u128(0x6175_6469_745f_6163_6374_5f65_7674_5f35);

const SOURCE: &str = "account";

// ── Wire schema (hand-matched to account's published JSON) ─────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountEventWire {
    AccountCreated(AccountCreatedWire),
    EmailChanged(EmailChangedWire),
    GdprDeletionRequested(GdprDeletionRequestedWire),
    GdprDataExportRequested(GdprDataExportRequestedWire),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountCreatedWire {
    pub account_id: String,
    /// PII — sealed, never in attributes.
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub status: String,
    /// PII (location) — sealed.
    #[serde(default)]
    pub country_of_residence: Option<String>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailChangedWire {
    pub account_id: String,
    pub old_email: String,
    pub new_email: String,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GdprDeletionRequestedWire {
    pub account_id: String,
    #[serde(default)]
    pub retention_days: u32,
    pub scheduled_deletion_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GdprDataExportRequestedWire {
    pub account_id: String,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub correlation_id: String,
}

impl AccountCreatedWire {
    /// The plaintext PII to seal (the consumer encrypts this before mapping).
    pub fn pii_plaintext(&self) -> String {
        format!("email={};country={}", self.email, self.country_of_residence.as_deref().unwrap_or(""))
    }
}

impl EmailChangedWire {
    pub fn pii_plaintext(&self) -> String {
        format!("old_email={};new_email={}", self.old_email, self.new_email)
    }
}

// ── Mapping ────────────────────────────────────────────────────────────────────

/// Account creation → an `Authorization` record carrying the new account's PII in
/// a sealed envelope (never in attributes). The platform is the acting principal.
pub fn map_account_created(
    wire: &AccountCreatedWire,
    sealed_pii: PiiEnvelope,
) -> Result<AuditEvent, AuditError> {
    let mut attributes = BTreeMap::new();
    if !wire.role.is_empty() {
        attributes.insert("role".to_owned(), wire.role.clone());
    }
    if !wire.status.is_empty() {
        attributes.insert("status".to_owned(), wire.status.clone());
    }

    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&["account.created", &wire.account_id]))?,
        category: EventCategory::Authorization,
        subject: Some(SubjectPseudonym::new(wire.account_id.clone())?),
        tenant: None,
        actor: Actor::new(ActorType::System, ActorPseudonym::new(SOURCE)?, String::new()),
        action: "account.created".to_owned(),
        resource: ResourceRef::new("account", wire.account_id.clone()),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::Contract,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: Some(sealed_pii),
        attributes,
    })
}

/// An email change → an `Authentication` record (a security-relevant contact
/// change) with the old/new email sealed. The user is the acting principal.
pub fn map_email_changed(
    wire: &EmailChangedWire,
    sealed_pii: PiiEnvelope,
) -> Result<AuditEvent, AuditError> {
    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&["account.email_changed", &wire.account_id, &wire.occurred_at.to_rfc3339()]))?,
        category: EventCategory::Authentication,
        subject: Some(SubjectPseudonym::new(wire.account_id.clone())?),
        tenant: None,
        actor: Actor::new(ActorType::User, ActorPseudonym::new(wire.account_id.clone())?, String::new()),
        action: "account.email_changed".to_owned(),
        resource: ResourceRef::new("account", wire.account_id.clone()),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::Contract,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: Some(sealed_pii),
        attributes: BTreeMap::new(),
    })
}

/// A GDPR Art. 17 erasure request → a `DataErasure` record. (The consumer also
/// triggers the actual crypto-shred of the subject.)
pub fn map_gdpr_deletion_requested(
    wire: &GdprDeletionRequestedWire,
) -> Result<AuditEvent, AuditError> {
    let mut attributes = BTreeMap::new();
    attributes.insert("retention_days".to_owned(), wire.retention_days.to_string());
    attributes.insert("scheduled_deletion_at".to_owned(), wire.scheduled_deletion_at.to_rfc3339());

    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&["account.gdpr_deletion_requested", &wire.account_id]))?,
        category: EventCategory::DataErasure,
        subject: Some(SubjectPseudonym::new(wire.account_id.clone())?),
        tenant: None,
        actor: Actor::new(ActorType::User, ActorPseudonym::new(wire.account_id.clone())?, String::new()),
        action: "account.gdpr_deletion_requested".to_owned(),
        resource: ResourceRef::new("account", wire.account_id.clone()),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: None,
        attributes,
    })
}

/// A GDPR Art. 15/20 export request → a `DataExport` record.
pub fn map_gdpr_data_export_requested(
    wire: &GdprDataExportRequestedWire,
) -> Result<AuditEvent, AuditError> {
    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(derive_id(&["account.gdpr_data_export_requested", &wire.account_id]))?,
        category: EventCategory::DataExport,
        subject: Some(SubjectPseudonym::new(wire.account_id.clone())?),
        tenant: None,
        actor: Actor::new(ActorType::User, ActorPseudonym::new(wire.account_id.clone())?, String::new()),
        action: "account.gdpr_data_export_requested".to_owned(),
        resource: ResourceRef::new("account", wire.account_id.clone()),
        outcome: Outcome::Executed,
        lawful_basis: LawfulBasis::LegalObligation,
        source_service: SOURCE.to_owned(),
        correlation_id: wire.correlation_id.clone(),
        occurred_at: wire.occurred_at,
        pii: None,
        attributes: BTreeMap::new(),
    })
}

fn derive_id(parts: &[&str]) -> String {
    Uuid::new_v5(&NS_AUDIT_ACCOUNT, parts.join(":").as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SubjectKeyRef;

    fn sealed() -> PiiEnvelope {
        PiiEnvelope::sealed(
            SubjectKeyRef::new("dek:acc").unwrap(),
            b"ct".to_vec(),
            b"n".to_vec(),
            "AES-256-GCM",
        )
    }

    fn created() -> AccountCreatedWire {
        serde_json::from_value(serde_json::json!({
            "type": "account_created",
            "account_id": "11111111-1111-1111-1111-111111111111",
            "identity_id": "kc-1",
            "email": "user@example.com",
            "role": "user",
            "status": "pending_verification",
            "country_of_residence": "FR",
            "occurred_at": "2026-06-27T12:00:00Z",
            "correlation_id": "22222222-2222-2222-2222-222222222222"
        }))
        .map(|w| match w {
            AccountEventWire::AccountCreated(c) => c,
            _ => panic!("expected AccountCreated"),
        })
        .unwrap()
    }

    #[test]
    fn account_created_seals_pii_and_keeps_it_out_of_attributes() {
        let wire = created();
        assert_eq!(wire.pii_plaintext(), "email=user@example.com;country=FR");
        let event = map_account_created(&wire, sealed()).unwrap();
        assert_eq!(event.category(), EventCategory::Authorization);
        assert_eq!(event.action(), "account.created");
        assert!(event.has_pii());
        // The email never appears in cleartext attributes.
        assert!(!event.attributes().values().any(|v| v.contains("user@example.com")));
        assert_eq!(event.attributes().get("role").unwrap(), "user");
    }

    #[test]
    fn email_changed_seals_both_addresses() {
        let wire: AccountEventWire = serde_json::from_value(serde_json::json!({
            "type": "email_changed",
            "account_id": "11111111-1111-1111-1111-111111111111",
            "old_email": "a@x.com",
            "new_email": "b@x.com",
            "occurred_at": "2026-06-27T12:00:00Z",
            "correlation_id": "22222222-2222-2222-2222-222222222222"
        }))
        .unwrap();
        let wire = match wire {
            AccountEventWire::EmailChanged(e) => e,
            _ => panic!(),
        };
        assert_eq!(wire.pii_plaintext(), "old_email=a@x.com;new_email=b@x.com");
        let event = map_email_changed(&wire, sealed()).unwrap();
        assert_eq!(event.category(), EventCategory::Authentication);
        assert!(event.has_pii());
        assert!(!event.attributes().values().any(|v| v.contains("@x.com")));
    }

    #[test]
    fn gdpr_deletion_maps_to_data_erasure() {
        let wire: AccountEventWire = serde_json::from_value(serde_json::json!({
            "type": "gdpr_deletion_requested",
            "account_id": "11111111-1111-1111-1111-111111111111",
            "retention_days": 30,
            "scheduled_deletion_at": "2026-07-27T12:00:00Z",
            "occurred_at": "2026-06-27T12:00:00Z",
            "correlation_id": "22222222-2222-2222-2222-222222222222"
        }))
        .unwrap();
        let wire = match wire {
            AccountEventWire::GdprDeletionRequested(g) => g,
            _ => panic!(),
        };
        let event = map_gdpr_deletion_requested(&wire).unwrap();
        assert_eq!(event.category(), EventCategory::DataErasure);
        assert_eq!(event.action(), "account.gdpr_deletion_requested");
        assert_eq!(event.lawful_basis(), LawfulBasis::LegalObligation);
        assert_eq!(event.attributes().get("retention_days").unwrap(), "30");
    }

    #[test]
    fn gdpr_export_maps_to_data_export() {
        let wire = GdprDataExportRequestedWire {
            account_id: "11111111-1111-1111-1111-111111111111".to_owned(),
            occurred_at: DateTime::parse_from_rfc3339("2026-06-27T12:00:00Z").unwrap().with_timezone(&Utc),
            correlation_id: String::new(),
        };
        let event = map_gdpr_data_export_requested(&wire).unwrap();
        assert_eq!(event.category(), EventCategory::DataExport);
    }

    #[test]
    fn lifecycle_events_decode_to_other() {
        for ty in ["account.suspended", "role_assigned", "mfa_enrolled", "password_changed"] {
            let wire: AccountEventWire =
                serde_json::from_value(serde_json::json!({ "type": ty, "account_id": "x" })).unwrap();
            assert!(matches!(wire, AccountEventWire::Other), "{ty} should be Other");
        }
    }
}
