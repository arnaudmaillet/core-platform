use async_trait::async_trait;

use crate::domain::SubjectKeyRef;
use crate::error::AuditError;

/// Custody of the per-subject data-encryption keys (DEKs) — the engine of
/// crypto-shred erasure. The concrete adapter (Phase 4) is KMS/HSM under an IAM
/// principal *separate* from the ledger's, so the database operator cannot read or
/// destroy keys and the key-holder cannot rewrite the chain.
///
/// Erasure is [`destroy_subject_key`](KeyVault::destroy_subject_key): the DEK is
/// irreversibly destroyed, rendering every PII envelope encrypted under it
/// permanently undecryptable, while the ledger rows (and thus the chain) are left
/// untouched. [`key_exists`](KeyVault::key_exists) is the read-time check that
/// tells the read path whether a record's PII is still readable or has been shred.
#[async_trait]
pub trait KeyVault: Send + Sync + 'static {
    /// Crypto-shred: irreversibly destroy a subject's DEK. Idempotent (destroying
    /// an absent key is `Ok`). An incomplete destruction is `AUD-5003`
    /// (CryptoShredFailed, retryable); an unreachable vault is `AUD-4003`.
    async fn destroy_subject_key(&self, key_ref: &SubjectKeyRef) -> Result<(), AuditError>;

    /// Whether a subject's DEK still exists. `false` means the subject was erased
    /// and the corresponding PII is undecryptable (`AUD-5004` on a read attempt).
    async fn key_exists(&self, key_ref: &SubjectKeyRef) -> Result<bool, AuditError>;
}
