use async_trait::async_trait;

use crate::domain::{PiiEnvelope, SubjectPseudonym};
use crate::error::AuditError;

/// Seals PII (e.g. a moderation rationale, the DSA statement-of-reasons) into a
/// crypto-shreddable [`PiiEnvelope`] under the subject's data-encryption key.
///
/// The adapter mints-or-fetches the per-subject DEK (wrapped under a service KEK
/// and persisted in the key vault) and AEAD-encrypts the plaintext. The plane only
/// ever holds the ciphertext thereafter. Erasure is then
/// [`KeyVault::destroy_subject_key`](super::KeyVault::destroy_subject_key): once
/// the wrapped DEK is destroyed, every envelope sealed for that subject is
/// permanently undecryptable — while the records and the hash chain stay intact.
///
/// This is the only seam through which plaintext PII enters the service; keeping it
/// behind a port means the production KMS adapter swaps in without touching the
/// ingest path.
#[async_trait]
pub trait SubjectCipher: Send + Sync + 'static {
    async fn seal(
        &self,
        subject: &SubjectPseudonym,
        plaintext: &str,
    ) -> Result<PiiEnvelope, AuditError>;
}
