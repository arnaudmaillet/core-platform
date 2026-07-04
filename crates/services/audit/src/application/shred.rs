use std::sync::Arc;

use crate::application::port::{Clock, KeyVault};
use crate::domain::{LegalHold, SubjectKeyRef, SubjectPseudonym, authorize_erasure};
use crate::error::AuditError;

/// The crypto-shred use case — how a GDPR Art. 17 erasure is executed without
/// breaking the audit trail. Erasure is *key destruction*, not record deletion:
/// destroying a subject's DEK renders their PII envelopes permanently
/// undecryptable while every ledger row, and the whole hash chain, stay intact and
/// verifiable.
///
/// Two gates, in order: lawful retention wins (an active legal hold blocks the
/// shred, `AUD-5002`), then the vault destroys the key. The handler never touches
/// the ledger — that is precisely what keeps the evidence intact.
pub struct CryptoShredHandler {
    key_vault: Arc<dyn KeyVault>,
    clock: Arc<dyn Clock>,
}

impl CryptoShredHandler {
    pub fn new(key_vault: Arc<dyn KeyVault>, clock: Arc<dyn Clock>) -> Self {
        Self { key_vault, clock }
    }

    /// Erase a subject by destroying their per-subject DEK, unless an active legal
    /// hold covers them. `holds` is the set of holds the worker resolved for this
    /// subject (themselves recorded as audit events).
    pub async fn shred(
        &self,
        subject: &SubjectPseudonym,
        key_ref: &SubjectKeyRef,
        holds: &[LegalHold],
    ) -> Result<(), AuditError> {
        // Lawful-retention override (GDPR Art. 17(3)) — checked before any
        // destructive action.
        authorize_erasure(subject, holds, self.clock.now())?;
        // Irreversible: the PII is now undecryptable; the chain is untouched.
        self.key_vault.destroy_subject_key(key_ref).await
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use error::AppError;

    use super::*;
    use crate::application::fakes::Fixture;

    fn subject() -> SubjectPseudonym {
        SubjectPseudonym::new("7f3a").unwrap()
    }

    fn key() -> SubjectKeyRef {
        SubjectKeyRef::new("dek:7f3a").unwrap()
    }

    #[tokio::test]
    async fn shred_destroys_the_subject_key() {
        let fx = Fixture::new();
        fx.key_vault.seed_key("dek:7f3a");
        assert!(fx.key_vault.exists("dek:7f3a"));

        fx.shred().shred(&subject(), &key(), &[]).await.unwrap();

        // Key gone → the subject's PII is now permanently undecryptable.
        assert!(!fx.key_vault.exists("dek:7f3a"));
    }

    #[tokio::test]
    async fn active_legal_hold_blocks_the_shred() {
        let fx = Fixture::new();
        fx.key_vault.seed_key("dek:7f3a");
        let hold = LegalHold::placed("lh-1", subject(), fx.now() - Duration::hours(1));

        let err = fx
            .shred()
            .shred(&subject(), &key(), std::slice::from_ref(&hold))
            .await
            .unwrap_err();

        assert_eq!(err.error_code(), "AUD-5002");
        // The key is untouched — lawful retention won.
        assert!(fx.key_vault.exists("dek:7f3a"));
    }

    #[tokio::test]
    async fn vault_outage_propagates_as_retryable() {
        let fx = Fixture::new();
        fx.key_vault.seed_key("dek:7f3a");
        fx.key_vault.set_unavailable(true);

        let err = fx.shred().shred(&subject(), &key(), &[]).await.unwrap_err();
        assert_eq!(err.error_code(), "AUD-4003");
        assert!(err.is_retryable());
    }
}
