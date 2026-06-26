//! The AES-256-GCM [`SubjectCipher`] adapter — the audit-side rationale-sealer and
//! the first concrete piece of the (otherwise deferred) KMS story.
//!
//! Envelope encryption: a random per-subject **DEK** encrypts the PII; the DEK is
//! wrapped under the service **KEK** and the wrapped DEK is stored in
//! `subject_keys` (one DEK per subject, get-or-create). The KEK lives in audit's
//! environment, never in the database — so the ledger operator alone cannot
//! decrypt. Crypto-shred ([`PgKeyVault::destroy_subject_key`] = delete the row)
//! destroys the wrapped DEK, making every envelope for that subject permanently
//! undecryptable.
//!
//! **v1 / deferral:** the KEK is read from env. Production hands KEK custody (the
//! wrap/unwrap) to KMS/HSM with no schema change — the wrapped-DEK column and this
//! port stay as-is.

use async_trait::async_trait;
use chrono::Utc;
use postgres_storage::TransactionManager;

use crate::application::port::SubjectCipher;
use crate::domain::{PiiEnvelope, SubjectKeyRef, SubjectPseudonym};
use crate::error::AuditError;
use crate::infrastructure::envelope::{self, KEY_LEN};

/// Get-or-create the wrapped DEK for a subject. Fills a NULL/absent row exactly
/// once (a lost race keeps the winner's DEK — never overwrites an existing one, so
/// prior ciphertexts stay decryptable), then the caller re-reads the canonical DEK.
const UPSERT_DEK_SQL: &str = r#"
INSERT INTO subject_keys (key_ref, created_at_ms, wrapped_dek, wrap_nonce)
VALUES ($1, $2, $3, $4)
ON CONFLICT (key_ref) DO UPDATE
    SET wrapped_dek = EXCLUDED.wrapped_dek, wrap_nonce = EXCLUDED.wrap_nonce
    WHERE subject_keys.wrapped_dek IS NULL
"#;

const FETCH_DEK_SQL: &str =
    "SELECT wrapped_dek, wrap_nonce FROM subject_keys WHERE key_ref = $1";

/// `(wrapped_dek, wrap_nonce)` — both nullable (a row may pre-exist without a DEK).
type WrappedDekRow = (Option<Vec<u8>>, Option<Vec<u8>>);

pub struct AesGcmSubjectCipher {
    tx: TransactionManager,
    kek: [u8; KEY_LEN],
}

impl AesGcmSubjectCipher {
    pub fn new(tx: TransactionManager, kek: [u8; KEY_LEN]) -> Self {
        Self { tx, kek }
    }

    /// The per-subject key reference — all of a subject's PII shares one DEK, so a
    /// single shred erases all of it.
    fn key_ref(subject: &SubjectPseudonym) -> String {
        format!("dek:{}", subject.as_str())
    }

    /// Resolve the subject's plaintext DEK, creating + wrapping one on first use.
    async fn dek_for(&self, key_ref: &str) -> Result<[u8; KEY_LEN], AuditError> {
        if let Some(dek) = self.fetch_dek(key_ref).await? {
            return Ok(dek);
        }
        // Create + wrap a fresh DEK; the upsert fills a NULL/absent row only.
        let dek = envelope::random_key()?;
        let wrapped = envelope::seal(&self.kek, &dek)?;
        sqlx::query(UPSERT_DEK_SQL)
            .bind(key_ref)
            .bind(Utc::now().timestamp_millis())
            .bind(wrapped.ciphertext)
            .bind(wrapped.nonce)
            .execute(self.tx.pool())
            .await
            .map_err(|_| AuditError::KeyVaultUnavailable)?;
        // Re-read the canonical DEK (ours, or a racing writer's that won).
        self.fetch_dek(key_ref)
            .await?
            .ok_or(AuditError::KeyVaultUnavailable)
    }

    async fn fetch_dek(&self, key_ref: &str) -> Result<Option<[u8; KEY_LEN]>, AuditError> {
        let row: Option<WrappedDekRow> = sqlx::query_as(FETCH_DEK_SQL)
            .bind(key_ref)
            .fetch_optional(self.tx.pool())
            .await
            .map_err(|_| AuditError::KeyVaultUnavailable)?;

        match row {
            Some((Some(wrapped), Some(wrap_nonce))) => {
                let dek = envelope::open(&self.kek, &wrapped, &wrap_nonce)?;
                let dek: [u8; KEY_LEN] = dek.try_into().map_err(|_| AuditError::DomainViolation {
                    field: "dek".to_owned(),
                    message: "unwrapped DEK has the wrong length".to_owned(),
                })?;
                Ok(Some(dek))
            }
            // No row, or a row without a wrapped DEK yet (e.g. legacy/seed).
            _ => Ok(None),
        }
    }
}

#[async_trait]
impl SubjectCipher for AesGcmSubjectCipher {
    async fn seal(
        &self,
        subject: &SubjectPseudonym,
        plaintext: &str,
    ) -> Result<PiiEnvelope, AuditError> {
        let key_ref = Self::key_ref(subject);
        let dek = self.dek_for(&key_ref).await?;
        let sealed = envelope::seal(&dek, plaintext.as_bytes())?;
        Ok(PiiEnvelope::sealed(
            SubjectKeyRef::new(key_ref)?,
            sealed.ciphertext,
            sealed.nonce,
            envelope::ALGORITHM,
        ))
    }
}
