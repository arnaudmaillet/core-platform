//! Issue #482 — crypto-shred end-to-end against the **KMS-backed** subject cipher,
//! over real Postgres. The KMS calls are mocked (no LocalStack needed in CI), but
//! the DEK store, the hash-chained ledger and the shred are the real adapters.
//!
//! What this proves: when KMS holds KEK custody, sealing a subject's PII mints a DEK
//! and asks KMS to *wrap* it (the opaque blob — never a raw KEK — lands in
//! `subject_keys`); the record chains as normal; and crypto-shred (delete the
//! wrapped-DEK row) makes the PII permanently unreadable while the chain still
//! verifies. The `subject_keys` schema is unchanged from the env-KEK path.

use std::sync::Mutex;

use async_trait::async_trait;

use audit::application::IntegrityStatus;
use audit::application::port::SubjectCipher;
use audit::domain::{EventCategory, SubjectKeyRef, SubjectPseudonym};
use audit::error::AuditError;
use audit::infrastructure::{KmsCipher, KmsSubjectCipher};

use postgres_storage::TransactionManager;

use crate::audit_it::harness::{Harness, fresh_tenant, partition_for, pii_event};

/// A stand-in for KMS: "wrapping" prefixes a marker (so we can assert the blob is
/// the KMS ciphertext, not a raw key) and counts the calls. The raw DEK is only
/// ever the plaintext argument — it never lands in the store.
#[derive(Default)]
struct FakeKms {
    encrypts: Mutex<usize>,
}

const MARKER: &[u8] = b"KMSWRAP::";

#[async_trait]
impl KmsCipher for FakeKms {
    async fn encrypt(&self, _key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, AuditError> {
        *self.encrypts.lock().unwrap() += 1;
        let mut blob = MARKER.to_vec();
        blob.extend_from_slice(plaintext);
        Ok(blob)
    }

    async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, AuditError> {
        ciphertext
            .strip_prefix(MARKER)
            .map(<[u8]>::to_vec)
            .ok_or(AuditError::KeyVaultUnavailable)
    }
}

#[tokio::test]
async fn crypto_shred_works_against_the_kms_backed_cipher() {
    let h = Harness::start().await;

    let kms = std::sync::Arc::new(FakeKms::default());
    let cipher = KmsSubjectCipher::new(
        TransactionManager::new(h.pool.clone()),
        kms.clone(),
        "alias/audit-dek".to_owned(),
    );

    let tenant = fresh_tenant();
    let subject = SubjectPseudonym::new("subj-1").unwrap();

    // First seal mints a DEK and KMS-wraps it; the wrapped blob persists in real PG.
    let envelope = cipher.seal(&subject, "violates harassment policy 3.2").await.unwrap();
    assert!(!envelope.ciphertext().is_empty());
    assert_eq!(*kms.encrypts.lock().unwrap(), 1, "the DEK was wrapped via KMS");

    let key_ref = format!("dek:{}", subject.as_str());
    let key = SubjectKeyRef::new(key_ref.clone()).unwrap();

    // The stored wrapped DEK is the KMS ciphertext blob, never a raw key.
    let wrapped: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT wrapped_dek FROM subject_keys WHERE key_ref = $1")
            .bind(&key_ref)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert!(
        wrapped.as_deref().unwrap().starts_with(MARKER),
        "subject_keys must hold the KMS-wrapped DEK, not raw key material"
    );

    // Chain a record that references the sealed PII, then verify it stands.
    h.ingest().ingest(pii_event(&tenant, "kms-pii-1", &key_ref)).await.unwrap();
    let partition = partition_for(&tenant, EventCategory::DataAccess);
    assert!(h.key_vault.key_exists(&key).await.unwrap());
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );

    // Crypto-shred: destroy the wrapped DEK. Even KMS can no longer recover it.
    h.shred().shred(&subject, &key, &[]).await.unwrap();

    // The DEK is gone (PII permanently undecryptable)...
    assert!(!h.key_vault.key_exists(&key).await.unwrap());
    // ...but the ledger row is untouched, so the chain STILL verifies.
    assert_eq!(
        h.verify().verify_partition(&partition).await.unwrap().status,
        IntegrityStatus::Verified
    );
}
