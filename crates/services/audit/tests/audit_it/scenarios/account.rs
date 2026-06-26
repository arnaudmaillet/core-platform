//! Live account→audit path over real Postgres + MinIO: account PII is sealed and
//! chained, and a GDPR deletion request crypto-shreds the subject — closing the
//! Art. 17 loop end to end (the rationale/PII becomes unreadable, the chain still
//! verifies). Audit's suite boots no Kafka, so we drive seal → map → ingest (and
//! the shred) directly, as the consumer would.

use audit::application::IntegrityStatus;
use audit::domain::{EventCategory, PartitionKey, SubjectKeyRef, SubjectPseudonym};
use audit::infrastructure::account_decode::{AccountCreatedWire, GdprDeletionRequestedWire};
use audit::infrastructure::{map_account_created, map_gdpr_deletion_requested};
use uuid::Uuid;

use crate::audit_it::harness::{Harness, at};

#[tokio::test]
async fn account_pii_is_sealed_and_gdpr_deletion_shreds_the_subject() {
    let h = Harness::start().await;
    let account = Uuid::now_v7().to_string();
    let subject = SubjectPseudonym::new(account.clone()).unwrap();
    let key = SubjectKeyRef::new(format!("dek:{account}")).unwrap();

    // 1. account.created — PII sealed over real Postgres, then chained.
    let created = AccountCreatedWire {
        account_id: account.clone(),
        email: "user@example.com".to_owned(),
        role: "user".to_owned(),
        status: "pending_verification".to_owned(),
        country_of_residence: Some("FR".to_owned()),
        occurred_at: at(1_750_000_000_000),
        correlation_id: Uuid::now_v7().to_string(),
    };
    let pii = h.cipher.seal(&subject, &created.pii_plaintext()).await.unwrap();
    h.ingest().ingest(map_account_created(&created, pii).unwrap()).await.unwrap();

    // The subject's DEK now exists (the PII is recoverable by an authorized reader).
    assert!(h.key_vault.key_exists(&key).await.unwrap());

    // 2. account.gdpr_deletion_requested — chain the erasure record, then shred.
    let deletion = GdprDeletionRequestedWire {
        account_id: account.clone(),
        retention_days: 30,
        scheduled_deletion_at: at(1_752_592_000_000),
        occurred_at: at(1_750_000_001_000),
        correlation_id: Uuid::now_v7().to_string(),
    };
    h.ingest().ingest(map_gdpr_deletion_requested(&deletion).unwrap()).await.unwrap();
    h.shred().shred(&subject, &key, &[]).await.unwrap();

    // The DEK is gone → all of the subject's sealed PII is permanently unreadable.
    assert!(!h.key_vault.key_exists(&key).await.unwrap());

    // Both account records live in the tenant-less Authorization (account.created)
    // and DataErasure (the gdpr request) partitions; both still verify.
    let authz = PartitionKey::derive(None, EventCategory::Authorization);
    let erasure = PartitionKey::derive(None, EventCategory::DataErasure);
    assert_eq!(
        h.verify().verify_partition(&authz).await.unwrap().status,
        IntegrityStatus::Verified
    );
    assert_eq!(
        h.verify().verify_partition(&erasure).await.unwrap().status,
        IntegrityStatus::Verified
    );
}
