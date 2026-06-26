//! Live crypto-shred over real Postgres: destroying a subject's DEK erases their
//! PII while the hash chain stays intact and verifiable — the GDPR erasure ⇄ audit
//! reconciliation, end to end.

use audit::application::IntegrityStatus;
use audit::domain::{EventCategory, SubjectKeyRef, SubjectPseudonym};

use crate::audit_it::harness::{Harness, fresh_tenant, partition_for, pii_event};

#[tokio::test]
async fn crypto_shred_erases_pii_yet_chain_survives() {
    let h = Harness::start().await;
    let tenant = fresh_tenant();
    let key_ref = format!("dek:{tenant}");

    h.seed_key(&key_ref).await;
    h.ingest().ingest(pii_event(&tenant, "pii-1", &key_ref)).await.unwrap();

    let partition = partition_for(&tenant, EventCategory::DataAccess);
    let key = SubjectKeyRef::new(key_ref.clone()).unwrap();

    // Before: the key exists and the chain verifies.
    assert!(h.key_vault.key_exists(&key).await.unwrap());
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );

    // Erase the subject (no legal hold).
    h.shred()
        .shred(&SubjectPseudonym::new("subj-1").unwrap(), &key, &[])
        .await
        .unwrap();

    // After: the DEK is gone (PII permanently undecryptable)...
    assert!(!h.key_vault.key_exists(&key).await.unwrap());
    // ...but the ledger row is untouched, so the chain STILL verifies.
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );
}
