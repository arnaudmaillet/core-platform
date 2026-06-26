//! The pure proto mapping between the domain and the generated `audit.v1` wire
//! types — the one place that knows the `audit-api` representation, so the rest of
//! the service stays proto-free. Used by the gRPC surface (Phase 5): decode a
//! `RecordPrivilegedRequest` / `QueryRequest` into domain values, and build the
//! `RecordPrivilegedResponse` / `AuditRecord` / `VerifyIntegrityResponse` /
//! `ExportManifest` replies.
//!
//! Everything here is total and unit-tested; there is no I/O.

use std::collections::BTreeMap;

use audit_api as pb;
use chrono::{DateTime, Utc};

use crate::application::dto::{ExportManifest, IntegrityReport, IntegrityStatus, LedgerQuery, RecordProof};
use crate::domain::{
    Actor, ActorPseudonym, ActorType, AuditEvent, AuditRecord, EventCategory, EventId, LawfulBasis,
    NewAuditEvent, Outcome, PiiEnvelope, PrivilegedActionType, RecordHash, ResourceRef,
    SubjectKeyRef, SubjectPseudonym, TenantId,
};
use crate::error::AuditError;

// ── time ──────────────────────────────────────────────────────────────────────

fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(0, 0).expect("unix epoch is valid")
}

fn ts_to_pb(dt: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn ts_from_pb(ts: Option<&prost_types::Timestamp>) -> DateTime<Utc> {
    ts.and_then(|t| DateTime::from_timestamp(t.seconds, t.nanos.max(0) as u32))
        .unwrap_or_else(epoch)
}

// ── enums ───────────────────────────────────────────────────────────────────

fn category_to_pb(c: EventCategory) -> pb::EventCategory {
    match c {
        EventCategory::Authentication => pb::EventCategory::Authentication,
        EventCategory::Authorization => pb::EventCategory::Authorization,
        EventCategory::Moderation => pb::EventCategory::Moderation,
        EventCategory::Consent => pb::EventCategory::Consent,
        EventCategory::DataAccess => pb::EventCategory::DataAccess,
        EventCategory::DataExport => pb::EventCategory::DataExport,
        EventCategory::DataErasure => pb::EventCategory::DataErasure,
        EventCategory::PrivilegedAction => pb::EventCategory::PrivilegedAction,
        EventCategory::Retention => pb::EventCategory::Retention,
    }
}

fn category_from_pb(value: i32) -> Result<EventCategory, AuditError> {
    match pb::EventCategory::try_from(value) {
        Ok(pb::EventCategory::Authentication) => Ok(EventCategory::Authentication),
        Ok(pb::EventCategory::Authorization) => Ok(EventCategory::Authorization),
        Ok(pb::EventCategory::Moderation) => Ok(EventCategory::Moderation),
        Ok(pb::EventCategory::Consent) => Ok(EventCategory::Consent),
        Ok(pb::EventCategory::DataAccess) => Ok(EventCategory::DataAccess),
        Ok(pb::EventCategory::DataExport) => Ok(EventCategory::DataExport),
        Ok(pb::EventCategory::DataErasure) => Ok(EventCategory::DataErasure),
        Ok(pb::EventCategory::PrivilegedAction) => Ok(EventCategory::PrivilegedAction),
        Ok(pb::EventCategory::Retention) => Ok(EventCategory::Retention),
        Ok(pb::EventCategory::Unspecified) | Err(_) => Err(AuditError::UnknownEventCategory {
            category: value.to_string(),
        }),
    }
}

fn actor_type_to_pb(a: ActorType) -> pb::ActorType {
    match a {
        ActorType::User => pb::ActorType::User,
        ActorType::Admin => pb::ActorType::Admin,
        ActorType::Service => pb::ActorType::Service,
        ActorType::System => pb::ActorType::System,
    }
}

fn actor_type_from_pb(value: i32) -> ActorType {
    // An unrecognized/unspecified actor type is non-fatal — it defaults to System
    // (a machine principal); the record is still evidence.
    match pb::ActorType::try_from(value) {
        Ok(pb::ActorType::User) => ActorType::User,
        Ok(pb::ActorType::Admin) => ActorType::Admin,
        Ok(pb::ActorType::Service) => ActorType::Service,
        _ => ActorType::System,
    }
}

fn outcome_to_pb(o: Outcome) -> pb::Outcome {
    match o {
        Outcome::Permitted => pb::Outcome::Permitted,
        Outcome::Denied => pb::Outcome::Denied,
        Outcome::Executed => pb::Outcome::Executed,
        Outcome::Failed => pb::Outcome::Failed,
    }
}

fn outcome_from_pb(value: i32) -> Result<Outcome, AuditError> {
    match pb::Outcome::try_from(value) {
        Ok(pb::Outcome::Permitted) => Ok(Outcome::Permitted),
        Ok(pb::Outcome::Denied) => Ok(Outcome::Denied),
        Ok(pb::Outcome::Executed) => Ok(Outcome::Executed),
        Ok(pb::Outcome::Failed) => Ok(Outcome::Failed),
        Ok(pb::Outcome::Unspecified) | Err(_) => Err(AuditError::MalformedAuditEvent {
            reason: "unspecified outcome".to_owned(),
        }),
    }
}

fn lawful_basis_to_pb(b: LawfulBasis) -> pb::LawfulBasis {
    match b {
        LawfulBasis::Unspecified => pb::LawfulBasis::Unspecified,
        LawfulBasis::Consent => pb::LawfulBasis::Consent,
        LawfulBasis::Contract => pb::LawfulBasis::Contract,
        LawfulBasis::LegalObligation => pb::LawfulBasis::LegalObligation,
        LawfulBasis::VitalInterests => pb::LawfulBasis::VitalInterests,
        LawfulBasis::PublicTask => pb::LawfulBasis::PublicTask,
        LawfulBasis::LegitimateInterests => pb::LawfulBasis::LegitimateInterests,
    }
}

fn lawful_basis_from_pb(value: i32) -> LawfulBasis {
    match pb::LawfulBasis::try_from(value) {
        Ok(pb::LawfulBasis::Consent) => LawfulBasis::Consent,
        Ok(pb::LawfulBasis::Contract) => LawfulBasis::Contract,
        Ok(pb::LawfulBasis::LegalObligation) => LawfulBasis::LegalObligation,
        Ok(pb::LawfulBasis::VitalInterests) => LawfulBasis::VitalInterests,
        Ok(pb::LawfulBasis::PublicTask) => LawfulBasis::PublicTask,
        Ok(pb::LawfulBasis::LegitimateInterests) => LawfulBasis::LegitimateInterests,
        _ => LawfulBasis::Unspecified,
    }
}

/// Map the enrolled privileged-action type from the wire. An unspecified value is
/// rejected — the sync lane must name a recognized enrolled action.
pub fn privileged_action_from_pb(value: i32) -> Result<PrivilegedActionType, AuditError> {
    match pb::PrivilegedActionType::try_from(value) {
        Ok(pb::PrivilegedActionType::BreakGlassAccess) => {
            Ok(PrivilegedActionType::BreakGlassAccess)
        }
        Ok(pb::PrivilegedActionType::LegalHoldPlace) => Ok(PrivilegedActionType::LegalHoldPlace),
        Ok(pb::PrivilegedActionType::LegalHoldRelease) => {
            Ok(PrivilegedActionType::LegalHoldRelease)
        }
        Ok(pb::PrivilegedActionType::Unspecified) | Err(_) => {
            Err(AuditError::MalformedAuditEvent {
                reason: "unspecified privileged action".to_owned(),
            })
        }
    }
}

fn integrity_status_to_pb(s: IntegrityStatus) -> pb::IntegrityStatus {
    match s {
        IntegrityStatus::Verified => pb::IntegrityStatus::Verified,
        IntegrityStatus::HashMismatch => pb::IntegrityStatus::HashMismatch,
        IntegrityStatus::SequenceGap => pb::IntegrityStatus::SequenceGap,
        IntegrityStatus::CheckpointDivergence => pb::IntegrityStatus::CheckpointDivergence,
    }
}

// ── nested messages ───────────────────────────────────────────────────────────

fn actor_to_pb(a: &Actor) -> pb::Actor {
    pb::Actor {
        r#type: actor_type_to_pb(a.actor_type) as i32,
        actor_pseudonym: a.pseudonym.as_str().to_owned(),
        session_ref: a.session_ref.clone(),
    }
}

fn actor_from_pb(a: pb::Actor) -> Result<Actor, AuditError> {
    Ok(Actor::new(
        actor_type_from_pb(a.r#type),
        ActorPseudonym::new(a.actor_pseudonym)?,
        a.session_ref,
    ))
}

fn resource_to_pb(r: &ResourceRef) -> pb::ResourceRef {
    pb::ResourceRef {
        r#type: r.resource_type.clone(),
        id: r.id.clone(),
    }
}

fn pii_to_pb(p: &PiiEnvelope) -> pb::PiiEnvelope {
    pb::PiiEnvelope {
        subject_key_id: p.subject_key_ref().as_str().to_owned(),
        ciphertext: p.ciphertext().to_vec(),
        nonce: p.nonce().to_vec(),
        algorithm: p.algorithm().to_owned(),
    }
}

fn pii_from_pb(p: pb::PiiEnvelope) -> Result<PiiEnvelope, AuditError> {
    Ok(PiiEnvelope::sealed(
        SubjectKeyRef::new(p.subject_key_id)?,
        p.ciphertext,
        p.nonce,
        p.algorithm,
    ))
}

fn optional_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(value)
    }
}

// ── AuditEvent ⇄ pb ─────────────────────────────────────────────────────────

pub fn event_to_pb(event: &AuditEvent) -> pb::AuditEvent {
    pb::AuditEvent {
        event_id: event.event_id().as_str().to_owned(),
        category: category_to_pb(event.category()) as i32,
        subject_pseudonym: event.subject().map(|s| s.as_str().to_owned()).unwrap_or_default(),
        tenant_id: event.tenant().map(|t| t.as_str().to_owned()).unwrap_or_default(),
        actor: Some(actor_to_pb(event.actor())),
        action: event.action().to_owned(),
        resource: Some(resource_to_pb(event.resource())),
        outcome: outcome_to_pb(event.outcome()) as i32,
        lawful_basis: lawful_basis_to_pb(event.lawful_basis()) as i32,
        source_service: event.source_service().to_owned(),
        correlation_id: event.correlation_id().to_owned(),
        occurred_at: Some(ts_to_pb(event.occurred_at())),
        pii: event.pii().map(pii_to_pb),
        attributes: event.attributes().clone().into_iter().collect(),
    }
}

pub fn event_from_pb(pb: pb::AuditEvent) -> Result<AuditEvent, AuditError> {
    let category = category_from_pb(pb.category)?;
    let actor = pb
        .actor
        .ok_or_else(|| AuditError::MalformedAuditEvent {
            reason: "missing actor".to_owned(),
        })
        .and_then(actor_from_pb)?;
    let resource = pb
        .resource
        .map(|r| ResourceRef::new(r.r#type, r.id))
        .unwrap_or_else(|| ResourceRef::new(String::new(), String::new()));

    let attributes: BTreeMap<String, String> = pb.attributes.into_iter().collect();

    AuditEvent::try_new(NewAuditEvent {
        event_id: EventId::new(pb.event_id)?,
        category,
        subject: optional_id(pb.subject_pseudonym).map(SubjectPseudonym::new).transpose()?,
        tenant: optional_id(pb.tenant_id).map(TenantId::new).transpose()?,
        actor,
        action: pb.action,
        resource,
        outcome: outcome_from_pb(pb.outcome)?,
        lawful_basis: lawful_basis_from_pb(pb.lawful_basis),
        source_service: pb.source_service,
        correlation_id: pb.correlation_id,
        occurred_at: ts_from_pb(pb.occurred_at.as_ref()),
        pii: pb.pii.map(pii_from_pb).transpose()?,
        attributes,
    })
}

// ── outbound replies ──────────────────────────────────────────────────────────

pub fn record_to_pb(record: &AuditRecord) -> pb::AuditRecord {
    pb::AuditRecord {
        event: Some(event_to_pb(record.event())),
        partition_key: record.partition().as_str().to_owned(),
        sequence_no: record.sequence(),
        record_hash: record.record_hash().as_str().to_owned(),
        prev_hash: record.prev_hash().as_str().to_owned(),
        recorded_at: Some(ts_to_pb(record.recorded_at())),
        pii_erased: record.pii_erased(),
    }
}

pub fn proof_to_pb(proof: &RecordProof) -> pb::RecordPrivilegedResponse {
    pb::RecordPrivilegedResponse {
        event_id: proof.event_id.as_str().to_owned(),
        partition_key: proof.partition.as_str().to_owned(),
        sequence_no: proof.sequence,
        record_hash: proof.record_hash.as_str().to_owned(),
        committed_at: Some(ts_to_pb(proof.committed_at)),
    }
}

pub fn integrity_report_to_pb(report: &IntegrityReport) -> pb::VerifyIntegrityResponse {
    pb::VerifyIntegrityResponse {
        status: integrity_status_to_pb(report.status) as i32,
        verified_through_sequence: report.verified_through,
        checkpoint_root: report
            .checkpoint_root
            .as_ref()
            .map(RecordHash::as_str)
            .unwrap_or("")
            .to_owned(),
        divergence_at_sequence: report.divergence_at.unwrap_or(0),
        detail: String::new(),
    }
}

pub fn export_manifest_to_pb(manifest: &ExportManifest) -> pb::ExportManifest {
    pb::ExportManifest {
        export_id: manifest.export_id.clone(),
        record_count: manifest.record_count,
        content_hash: manifest.content_hash.as_str().to_owned(),
        artifact_ref: manifest.artifact_ref.clone(),
        generated_at: Some(ts_to_pb(manifest.generated_at)),
    }
}

// ── inbound query ─────────────────────────────────────────────────────────────

pub fn query_from_pb(pb: pb::QueryRequest) -> Result<LedgerQuery, AuditError> {
    Ok(LedgerQuery {
        subject: optional_id(pb.subject_pseudonym).map(SubjectPseudonym::new).transpose()?,
        tenant: optional_id(pb.tenant_id).map(TenantId::new).transpose()?,
        category: if pb.category == pb::EventCategory::Unspecified as i32 {
            None
        } else {
            Some(category_from_pb(pb.category)?)
        },
        from: pb.from.as_ref().map(|t| ts_from_pb(Some(t))),
        to: pb.to.as_ref().map(|t| ts_from_pb(Some(t))),
        limit: pb.page_size as usize,
    })
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::domain::event::fixtures;

    #[test]
    fn event_round_trips_through_pb() {
        let original =
            AuditEvent::try_new(fixtures::with_pii("evt-1")).unwrap();
        let restored = event_from_pb(event_to_pb(&original)).unwrap();
        // The hash is over the canonical content — equality of hashes proves the
        // round trip preserved every hashed field.
        assert_eq!(original.content_hash(), restored.content_hash());
    }

    #[test]
    fn unspecified_category_is_rejected() {
        let mut pb = event_to_pb(&AuditEvent::try_new(fixtures::draft("e", EventCategory::Moderation)).unwrap());
        pb.category = pb::EventCategory::Unspecified as i32;
        let err = event_from_pb(pb).unwrap_err();
        assert_eq!(err.error_code(), "AUD-1003");
    }

    #[test]
    fn missing_lawful_basis_on_pii_category_is_rejected() {
        let mut pb = event_to_pb(&AuditEvent::try_new(fixtures::draft("e", EventCategory::Consent)).unwrap());
        pb.lawful_basis = pb::LawfulBasis::Unspecified as i32;
        let err = event_from_pb(pb).unwrap_err();
        assert_eq!(err.error_code(), "AUD-1002");
    }

    #[test]
    fn privileged_action_maps_or_rejects_unspecified() {
        assert_eq!(
            privileged_action_from_pb(pb::PrivilegedActionType::BreakGlassAccess as i32).unwrap(),
            PrivilegedActionType::BreakGlassAccess
        );
        assert!(privileged_action_from_pb(pb::PrivilegedActionType::Unspecified as i32).is_err());
    }
}
