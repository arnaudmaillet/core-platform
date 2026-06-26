//! The canonical ledger over Postgres (sqlx) — the append-only, hash-chained
//! System of Record.
//!
//! Immutability is enforced at the database, not in code: the migrator grants this
//! service's role `INSERT` only — `UPDATE` and `DELETE` are **revoked**, so even a
//! compromised application credential can never rewrite or remove a record. The
//! chain (`prev_hash` / `record_hash` / monotonic per-partition `sequence_no`) then
//! makes any out-of-band tampering by a more-privileged operator *detectable*.
//!
//! Append is a compare-and-append: the new record's `(partition_key, sequence_no)`
//! is the primary key, and `ON CONFLICT (partition_key, sequence_no) DO NOTHING`
//! turns a lost race (another writer already took that slot) into a no-row result,
//! surfaced as `AUD-2003 ChainHeadConflict` so the caller re-reads the head and
//! re-chains. A separate `UNIQUE (event_id)` makes a true duplicate distinguishable
//! as `AUD-1004`.
//!
//! Schema (owned by the `migrator`, applied before rollout — Phase 6):
//! ```sql
//! CREATE TABLE audit_records (
//!   partition_key     text   NOT NULL,
//!   sequence_no       bigint NOT NULL,
//!   event_id          text   NOT NULL UNIQUE,
//!   record_hash       text   NOT NULL,
//!   subject_pseudonym text,
//!   tenant_id         text,
//!   category_tag      smallint NOT NULL,
//!   occurred_at_ms    bigint NOT NULL,
//!   recorded_at_ms    bigint NOT NULL,
//!   record_json       text   NOT NULL,
//!   PRIMARY KEY (partition_key, sequence_no));
//! CREATE INDEX audit_subject_idx ON audit_records (subject_pseudonym);
//! -- GRANT INSERT, SELECT ON audit_records TO audit_role;  -- NO update/delete
//! ```

use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::dto::LedgerQuery;
use crate::application::port::LedgerStore;
use crate::domain::{AuditRecord, ChainHead, EventId, PartitionKey, RecordHash};
use crate::error::AuditError;

const HEAD_SQL: &str = r#"
SELECT sequence_no, record_hash
FROM audit_records
WHERE partition_key = $1
ORDER BY sequence_no DESC
LIMIT 1
"#;

const LOOKUP_SQL: &str = r#"
SELECT record_json FROM audit_records WHERE event_id = $1
"#;

/// Compare-and-append: the PK slot guards the chain head; a taken slot yields no
/// row (a lost race), a duplicate `event_id` is a unique violation.
const APPEND_SQL: &str = r#"
INSERT INTO audit_records (
    partition_key, sequence_no, event_id, record_hash,
    subject_pseudonym, tenant_id, category_tag, occurred_at_ms, recorded_at_ms, record_json)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
ON CONFLICT (partition_key, sequence_no) DO NOTHING
RETURNING 1
"#;

const QUERY_SQL: &str = r#"
SELECT record_json FROM audit_records
WHERE ($1::text     IS NULL OR subject_pseudonym = $1)
  AND ($2::text     IS NULL OR tenant_id = $2)
  AND ($3::smallint IS NULL OR category_tag = $3)
  AND ($4::bigint   IS NULL OR occurred_at_ms >= $4)
  AND ($5::bigint   IS NULL OR occurred_at_ms <= $5)
ORDER BY recorded_at_ms
LIMIT $6
"#;

const READ_PARTITION_SQL: &str = r#"
SELECT record_json FROM audit_records WHERE partition_key = $1 ORDER BY sequence_no
"#;

const PARTITION_HEADS_SQL: &str = r#"
SELECT DISTINCT ON (partition_key) partition_key, record_hash
FROM audit_records
ORDER BY partition_key, sequence_no DESC
"#;

/// Hard cap on a page, applied when the caller passes 0 or an oversized limit.
const MAX_PAGE: i64 = 500;

pub struct PgLedger {
    tx: TransactionManager,
}

impl PgLedger {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

/// A read fault has no fallback (the ledger *is* the source of truth), so it is the
/// retryable unavailable variant.
fn unavailable(_e: sqlx::Error) -> AuditError {
    AuditError::LedgerStoreUnavailable
}

/// A stored record that won't deserialize is data corruption, not a transient
/// fault — surface it as a domain violation rather than masking it.
fn decode_row(json: &str) -> Result<AuditRecord, AuditError> {
    serde_json::from_str(json).map_err(|e| AuditError::DomainViolation {
        field: "record_json".to_owned(),
        message: format!("stored audit record failed to deserialize: {e}"),
    })
}

#[async_trait]
impl LedgerStore for PgLedger {
    async fn head(&self, partition: &PartitionKey) -> Result<ChainHead, AuditError> {
        let row: Option<(i64, String)> = sqlx::query_as(HEAD_SQL)
            .bind(partition.as_str())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(unavailable)?;
        Ok(match row {
            Some((seq, hash)) => ChainHead::from_parts(seq as u64, RecordHash::from_hex(hash)),
            None => ChainHead::genesis(),
        })
    }

    async fn lookup(&self, event_id: &EventId) -> Result<Option<AuditRecord>, AuditError> {
        let json: Option<String> = sqlx::query_scalar(LOOKUP_SQL)
            .bind(event_id.as_str())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(unavailable)?;
        json.as_deref().map(decode_row).transpose()
    }

    async fn append(
        &self,
        record: &AuditRecord,
        _expected_head: &ChainHead,
    ) -> Result<(), AuditError> {
        let event = record.event();
        let json = serde_json::to_string(record).map_err(|e| AuditError::DomainViolation {
            field: "record_json".to_owned(),
            message: format!("failed to serialize audit record: {e}"),
        })?;

        let inserted: Option<i32> = sqlx::query_scalar(APPEND_SQL)
            .bind(record.partition().as_str())
            .bind(record.sequence() as i64)
            .bind(event.event_id().as_str())
            .bind(record.record_hash().as_str())
            .bind(event.subject().map(|s| s.as_str()))
            .bind(event.tenant().map(|t| t.as_str()))
            .bind(i16::from(event.category().hash_tag()))
            .bind(event.occurred_at().timestamp_millis())
            .bind(record.recorded_at().timestamp_millis())
            .bind(json)
            .fetch_optional(self.tx.pool())
            .await
            .map_err(|e| {
                // A duplicate event_id is a true replay, not a head race.
                if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
                    AuditError::DuplicateEvent {
                        event_id: event.event_id().to_string(),
                    }
                } else {
                    AuditError::LedgerStoreUnavailable
                }
            })?;

        match inserted {
            Some(_) => Ok(()),
            // The PK slot was already taken — the head advanced under us.
            None => Err(AuditError::ChainHeadConflict {
                partition: record.partition().to_string(),
            }),
        }
    }

    async fn query(&self, spec: &LedgerQuery) -> Result<Vec<AuditRecord>, AuditError> {
        let limit = if spec.limit == 0 || spec.limit as i64 > MAX_PAGE {
            MAX_PAGE
        } else {
            spec.limit as i64
        };
        let rows: Vec<String> = sqlx::query_scalar(QUERY_SQL)
            .bind(spec.subject.as_ref().map(|s| s.as_str()))
            .bind(spec.tenant.as_ref().map(|t| t.as_str()))
            .bind(spec.category.map(|c| i16::from(c.hash_tag())))
            .bind(spec.from.map(|t| t.timestamp_millis()))
            .bind(spec.to.map(|t| t.timestamp_millis()))
            .bind(limit)
            .fetch_all(self.tx.pool())
            .await
            .map_err(unavailable)?;
        rows.iter().map(|j| decode_row(j)).collect()
    }

    async fn read_partition(
        &self,
        partition: &PartitionKey,
    ) -> Result<Vec<AuditRecord>, AuditError> {
        let rows: Vec<String> = sqlx::query_scalar(READ_PARTITION_SQL)
            .bind(partition.as_str())
            .fetch_all(self.tx.pool())
            .await
            .map_err(unavailable)?;
        rows.iter().map(|j| decode_row(j)).collect()
    }

    async fn partition_heads(&self) -> Result<Vec<(PartitionKey, RecordHash)>, AuditError> {
        let rows: Vec<(String, String)> = sqlx::query_as(PARTITION_HEADS_SQL)
            .fetch_all(self.tx.pool())
            .await
            .map_err(unavailable)?;
        Ok(rows
            .into_iter()
            .filter_map(|(p, h)| Some((PartitionKey::new(p).ok()?, RecordHash::from_hex(h))))
            .collect())
    }
}
