//! Plane C screen gate against the live Redis corpus: a seeded known-bad hash
//! blocks and records automated evidence (an append-only decision + a content
//! removal enforcement); clean content allows.

use crate::moderation_it::harness::{subject, Harness};
use moderation::infrastructure::grpc::proto;

#[tokio::test]
async fn known_bad_hash_blocks_and_records_evidence() {
    let h = Harness::start().await;
    h.seed_corpus("pdq", "badhash", &["csam"], "ncmec:42").await;

    let subj = subject(proto::EntityType::Media, "upload");
    let actor = subj.actor_id.clone();

    let resp = h
        .screen(subj, "pdq", "badhash", vec![proto::PolicyCategory::Csam as i32])
        .await
        .expect("screen");

    assert_eq!(resp.verdict, proto::ScreenVerdict::Block as i32);
    assert_eq!(resp.match_reference, "ncmec:42");
    assert!(resp.matched_categories.contains(&(proto::PolicyCategory::Csam as i32)));

    // Automated evidence is recorded: one decision + one active content removal.
    assert_eq!(h.count_decisions(&actor).await, 1);
    assert_eq!(h.count_enforcements(&actor, "active").await, 1);
}

#[tokio::test]
async fn clean_content_allows_and_records_nothing() {
    let h = Harness::start().await;
    let subj = subject(proto::EntityType::Media, "upload");
    let actor = subj.actor_id.clone();

    let resp = h
        .screen(subj, "pdq", "cleanhash", vec![proto::PolicyCategory::Csam as i32])
        .await
        .expect("screen");

    assert_eq!(resp.verdict, proto::ScreenVerdict::Allow as i32);
    assert_eq!(h.count_decisions(&actor).await, 0);
    assert_eq!(h.count_enforcements(&actor, "active").await, 0);
}
