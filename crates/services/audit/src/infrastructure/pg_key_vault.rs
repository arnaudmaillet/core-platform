//! The per-subject DEK custody over Postgres — the crypto-shred engine.
//!
//! **v1 placeholder, deliberate deferral.** Production custody is KMS/HSM under an
//! IAM principal *separate* from the ledger's, so the database operator can neither
//! read nor destroy keys (the integrity story depends on that separation — see the
//! README). This Postgres-backed registry implements the same [`KeyVault`] port so
//! the rest of the service is final; swapping in the KMS adapter is a composition
//! -root change, no domain or application change.
//!
//! Destroying a key here is a row `DELETE` — irreversible erasure of the subject's
//! DEK reference. The ledger rows (and the hash chain) are untouched, so the record
//! survives while the PII becomes undecryptable.
//!
//! Schema (owned by the `migrator`):
//! ```sql
//! CREATE TABLE subject_keys (key_ref text PRIMARY KEY, created_at_ms bigint NOT NULL);
//! ```

use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::KeyVault;
use crate::domain::SubjectKeyRef;
use crate::error::AuditError;

const DESTROY_SQL: &str = "DELETE FROM subject_keys WHERE key_ref = $1";
const EXISTS_SQL: &str = "SELECT EXISTS (SELECT 1 FROM subject_keys WHERE key_ref = $1)";

pub struct PgKeyVault {
    tx: TransactionManager,
}

impl PgKeyVault {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

fn unavailable(_e: sqlx::Error) -> AuditError {
    AuditError::KeyVaultUnavailable
}

#[async_trait]
impl KeyVault for PgKeyVault {
    async fn destroy_subject_key(&self, key_ref: &SubjectKeyRef) -> Result<(), AuditError> {
        // Idempotent: deleting an absent key affects zero rows and is still Ok.
        sqlx::query(DESTROY_SQL)
            .bind(key_ref.as_str())
            .execute(self.tx.pool())
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    async fn key_exists(&self, key_ref: &SubjectKeyRef) -> Result<bool, AuditError> {
        let exists: bool = sqlx::query_scalar(EXISTS_SQL)
            .bind(key_ref.as_str())
            .fetch_one(self.tx.pool())
            .await
            .map_err(unavailable)?;
        Ok(exists)
    }
}
