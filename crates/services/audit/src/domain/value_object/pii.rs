use serde::{Deserialize, Serialize};

use super::hash::CanonicalWriter;
use super::identity::SubjectKeyRef;

/// The crypto-shreddable PII envelope — how the audit plane reconciles the GDPR
/// "right to be forgotten" with the duty to keep a permanent, verifiable trail.
///
/// Any unavoidable personal data is stored **only** as `ciphertext`, encrypted
/// (out in the infrastructure tier) under a **per-subject** data-encryption key
/// named by `subject_key_ref`. The crucial property is that the hash chain covers
/// the **ciphertext**, never the plaintext. So:
///
/// Erasure (Art. 17) = destroy the per-subject DEK in the key vault. The
/// ciphertext here is left byte-for-byte unchanged — it just becomes permanently
/// undecryptable. Because the bytes the chain hashed never moved, the record, its
/// sequence and its hash remain intact and the chain still verifies. The proof
/// that an action occurred survives; the personal content of it evaporates.
///
/// This type therefore models a *sealed* envelope. It has no `decrypt` — the
/// domain never holds keys or plaintext; it only carries the ciphertext into the
/// canonical hash. Whether the DEK still exists (and thus whether this is
/// readable) is ledger metadata on [`crate::domain::AuditRecord::pii_erased`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiiEnvelope {
    subject_key_ref: SubjectKeyRef,
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    algorithm: String,
}

impl PiiEnvelope {
    /// Seal already-encrypted PII for inclusion in an event. The plaintext never
    /// reaches the domain — only the AEAD ciphertext, nonce, algorithm tag, and
    /// the per-subject key reference.
    pub fn sealed(
        subject_key_ref: SubjectKeyRef,
        ciphertext: Vec<u8>,
        nonce: Vec<u8>,
        algorithm: impl Into<String>,
    ) -> Self {
        Self {
            subject_key_ref,
            ciphertext,
            nonce,
            algorithm: algorithm.into(),
        }
    }

    pub fn subject_key_ref(&self) -> &SubjectKeyRef {
        &self.subject_key_ref
    }

    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    pub fn nonce(&self) -> &[u8] {
        &self.nonce
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Fold the envelope into the canonical hash buffer. Order is fixed; the
    /// **ciphertext** (not any plaintext) is what is hashed, which is exactly why
    /// destroying the key later does not disturb the chain.
    pub(crate) fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(self.subject_key_ref.as_str())
            .bytes(&self.ciphertext)
            .bytes(&self.nonce)
            .str(&self.algorithm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> PiiEnvelope {
        PiiEnvelope::sealed(
            SubjectKeyRef::new("dek:7f3a").unwrap(),
            b"ciphertext-bytes".to_vec(),
            b"nonce".to_vec(),
            "AES-256-GCM",
        )
    }

    #[test]
    fn exposes_ciphertext_never_plaintext() {
        let e = envelope();
        assert_eq!(e.ciphertext(), b"ciphertext-bytes");
        assert_eq!(e.algorithm(), "AES-256-GCM");
        assert_eq!(e.subject_key_ref().as_str(), "dek:7f3a");
    }

    #[test]
    fn canonical_form_is_deterministic() {
        let mut a = CanonicalWriter::new();
        envelope().write_canonical(&mut a);
        let mut b = CanonicalWriter::new();
        envelope().write_canonical(&mut b);
        assert_eq!(a.finish(), b.finish());
    }
}
