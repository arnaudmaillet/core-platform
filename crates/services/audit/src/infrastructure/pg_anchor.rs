//! The checkpoint anchor over Postgres — the bridge to the tamper witness.
//!
//! **v1 placeholder, deliberate deferral.** A real witness must live in a trust
//! domain the database operator does NOT control — an RFC 3161 trusted-timestamp
//! authority and/or a cross-account Object-Lock bucket — otherwise an operator who
//! tampers the ledger could forge the anchor too. This Postgres-backed table
//! implements the same [`CheckpointAnchor`] port so the verifier and worker loop
//! are final; swapping in the external witness adapter is a composition-root change.
//! Until then the global check still detects divergence between the live heads and
//! the last anchored root (it just isn't yet operator-proof).
//!
//! Schema (owned by the `migrator`):
//! ```sql
//! CREATE TABLE checkpoint_anchors (created_at_ms bigint PRIMARY KEY, checkpoint_json text NOT NULL);
//! ```

use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::CheckpointAnchor;
use crate::domain::MerkleCheckpoint;
use crate::error::AuditError;

const ANCHOR_SQL: &str = r#"
INSERT INTO checkpoint_anchors (created_at_ms, checkpoint_json)
VALUES ($1, $2)
ON CONFLICT (created_at_ms) DO NOTHING
"#;

const LATEST_SQL: &str = r#"
SELECT checkpoint_json FROM checkpoint_anchors ORDER BY created_at_ms DESC LIMIT 1
"#;

pub struct PgCheckpointAnchor {
    tx: TransactionManager,
}

impl PgCheckpointAnchor {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

fn unavailable(_e: sqlx::Error) -> AuditError {
    AuditError::AnchorWitnessUnavailable
}

#[async_trait]
impl CheckpointAnchor for PgCheckpointAnchor {
    async fn anchor(&self, checkpoint: &MerkleCheckpoint) -> Result<(), AuditError> {
        let json = serde_json::to_string(checkpoint)
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        sqlx::query(ANCHOR_SQL)
            .bind(checkpoint.created_at().timestamp_millis())
            .bind(json)
            .execute(self.tx.pool())
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    async fn latest_anchored(&self) -> Result<Option<MerkleCheckpoint>, AuditError> {
        let json: Option<String> = sqlx::query_scalar(LATEST_SQL)
            .fetch_optional(self.tx.pool())
            .await
            .map_err(unavailable)?;
        match json {
            Some(j) => Ok(Some(
                serde_json::from_str(&j).map_err(|_| AuditError::AnchorWitnessUnavailable)?,
            )),
            None => Ok(None),
        }
    }
}
