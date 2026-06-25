//! Plane A ingestion against the live Postgres store: a redelivered report
//! deduplicates onto one deterministic case while accruing signals; a self-report
//! is rejected; a classifier signal accrues onto the case.

use crate::moderation_it::harness::{subject, Harness};
use moderation::application::command::{IngestReportCommand, IngestSignalCommand};
use moderation::domain::value_object::{ActorId, Confidence, EntityType, PolicyCategory, SubjectRef};
use moderation::infrastructure::grpc::proto;
use uuid::Uuid;

/// Builds a domain `SubjectRef` matching a proto subject's identity.
fn domain_subject(p: &proto::SubjectRef) -> SubjectRef {
    SubjectRef::new(
        EntityType::Comment,
        p.entity_id.clone(),
        ActorId::try_from(p.actor_id.as_str()).unwrap(),
        p.surface.clone(),
    )
    .unwrap()
}

#[tokio::test]
async fn redelivered_report_dedups_onto_one_case() {
    let h = Harness::start().await;
    let subj = subject(proto::EntityType::Comment, "thread");
    let actor = subj.actor_id.clone();
    let domain = domain_subject(&subj);

    // Two different reporters report the same content.
    for reporter in [Uuid::now_v7(), Uuid::now_v7()] {
        h.ingest_report(IngestReportCommand {
            reporter_id: ActorId::from_uuid(reporter),
            subject: domain.clone(),
            category: PolicyCategory::Harassment,
            reason: "abusive".into(),
        })
        .await
        .expect("ingest report");
    }

    // One deterministic case, two accrued signals.
    assert_eq!(h.count_cases(&actor).await, 1, "deterministic case dedup");
    assert_eq!(h.count_signals(&actor).await, 2, "two report signals accrued");
}

#[tokio::test]
async fn self_report_is_rejected() {
    let h = Harness::start().await;
    let subj = subject(proto::EntityType::Comment, "thread");
    let domain = domain_subject(&subj);

    let err = h
        .ingest_report(IngestReportCommand {
            reporter_id: domain.actor_id(), // reporter == content author
            subject: domain.clone(),
            category: PolicyCategory::Spam,
            reason: "x".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, moderation::ModerationError::SelfReportRejected));
    assert_eq!(h.count_cases(&subj.actor_id).await, 0, "no case opened");
}

#[tokio::test]
async fn classifier_signal_accrues_onto_a_case() {
    let h = Harness::start().await;
    let subj = subject(proto::EntityType::Comment, "thread");
    let actor = subj.actor_id.clone();
    let domain = domain_subject(&subj);

    h.ingest_signal(IngestSignalCommand {
        subject: domain,
        source: "classifier:text-v2".into(),
        category: PolicyCategory::Hate,
        confidence: Confidence::new(0.93).unwrap(),
    })
    .await
    .expect("ingest signal");

    assert_eq!(h.count_cases(&actor).await, 1);
    assert_eq!(h.count_signals(&actor).await, 1);
}
