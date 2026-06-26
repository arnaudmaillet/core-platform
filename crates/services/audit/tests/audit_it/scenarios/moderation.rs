//! Live moderation→audit path over real Postgres + MinIO: a moderation decision's
//! rationale is sealed (real AES-GCM + a Postgres-backed wrapped DEK), chained, and
//! verifiable — and a crypto-shred of the subject leaves the chain intact while the
//! rationale becomes permanently unreadable.
//!
//! Audit's suite boots no Kafka, so the consumer is out of scope here; we drive the
//! same steps it would (seal → map → ingest) directly over the real adapters.

use audit::application::IntegrityStatus;
use audit::domain::{EventCategory, PartitionKey, SubjectKeyRef, SubjectPseudonym};
use audit::infrastructure::map_decision_recorded;
use audit::infrastructure::moderation_decode::{
    DecisionAuthorWire, DecisionRecordedWire, SubjectRefWire,
};
use uuid::Uuid;

use crate::audit_it::harness::{Harness, at};

fn decision_wire(actor_id: &str, rationale: &str) -> DecisionRecordedWire {
    DecisionRecordedWire {
        decision_id: Uuid::now_v7().to_string(),
        subject: SubjectRefWire {
            entity_type: "post".to_owned(),
            entity_id: "p1".to_owned(),
            actor_id: actor_id.to_owned(),
            surface: "feed".to_owned(),
        },
        author: DecisionAuthorWire::Reviewer("rev-1".to_owned()),
        action: "remove_content".to_owned(),
        category: "harassment".to_owned(),
        policy_version: "2026.06.1".to_owned(),
        rationale: rationale.to_owned(),
        reverses: None,
        occurred_at: at(1_750_000_000_000),
        correlation_id: Uuid::now_v7().to_string(),
    }
}

#[tokio::test]
async fn moderation_decision_is_sealed_chained_and_shred_preserves_chain() {
    let h = Harness::start().await;
    // A fresh subject per run — its DEK is isolated even though moderation records
    // share the (tenant-less) `_global` Moderation partition.
    let actor = Uuid::now_v7().to_string();
    let subject = SubjectPseudonym::new(actor.clone()).unwrap();
    let key = SubjectKeyRef::new(format!("dek:{actor}")).unwrap();

    let wire = decision_wire(&actor, "violates harassment policy clause 3.2");

    // Seal the rationale over real Postgres (mints + KEK-wraps a per-subject DEK),
    // map to a domain event, and chain it.
    let pii = h.cipher.seal(&subject, &wire.rationale).await.unwrap();
    let event = map_decision_recorded(&wire, pii).unwrap();
    h.ingest().ingest(event).await.unwrap();

    let partition = PartitionKey::derive(None, EventCategory::Moderation);
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );
    // The DEK exists → an authorized reader could still decrypt the rationale.
    assert!(h.key_vault.key_exists(&key).await.unwrap());

    // Crypto-shred the subject: the wrapped DEK is destroyed...
    h.shred().shred(&subject, &key, &[]).await.unwrap();
    assert!(!h.key_vault.key_exists(&key).await.unwrap());

    // ...the rationale is now permanently undecryptable, yet the chain STILL
    // verifies — the record's ciphertext (what the chain hashed) never moved.
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );
}

#[tokio::test]
async fn moderation_decision_replay_is_deduped() {
    let h = Harness::start().await;
    let actor = Uuid::now_v7().to_string();
    let subject = SubjectPseudonym::new(actor.clone()).unwrap();
    let wire = decision_wire(&actor, "violates policy");

    // The audit event id is a deterministic UUIDv5 of the decision id, so a
    // redelivery maps to the same id and the ledger dedupes it — even though each
    // seal produces a fresh envelope.
    let first = {
        let pii = h.cipher.seal(&subject, &wire.rationale).await.unwrap();
        h.ingest().ingest(map_decision_recorded(&wire, pii).unwrap()).await.unwrap()
    };
    let again = {
        let pii = h.cipher.seal(&subject, &wire.rationale).await.unwrap();
        h.ingest().ingest(map_decision_recorded(&wire, pii).unwrap()).await.unwrap()
    };

    assert!(!first.is_duplicate());
    assert!(again.is_duplicate());
    assert_eq!(first.proof(), again.proof());
}

#[tokio::test]
async fn cipher_reuses_per_subject_dek_with_fresh_nonces() {
    let h = Harness::start().await;
    let actor = Uuid::now_v7().to_string();
    let subject = SubjectPseudonym::new(actor.clone()).unwrap();
    let key = SubjectKeyRef::new(format!("dek:{actor}")).unwrap();

    let a = h.cipher.seal(&subject, "reason A").await.unwrap();
    let b = h.cipher.seal(&subject, "reason B").await.unwrap();

    // One DEK per subject (a single shred erases all their PII)...
    assert_eq!(a.subject_key_ref(), b.subject_key_ref());
    // ...but a fresh nonce + distinct ciphertext per seal (semantic security).
    assert_ne!(a.nonce(), b.nonce());
    assert_ne!(a.ciphertext(), b.ciphertext());

    assert!(h.key_vault.key_exists(&key).await.unwrap());
    h.shred().shred(&subject, &key, &[]).await.unwrap();
    assert!(!h.key_vault.key_exists(&key).await.unwrap());
}
