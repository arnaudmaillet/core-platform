//! The warm counter tier over Postgres (sqlx) — the auditable durable totals and
//! the idempotency ledger.
//!
//! Idempotency is a single atomic statement. A window is recorded in
//! `counter_windows` with `ON CONFLICT DO NOTHING`; the total in `counter_totals`
//! is advanced **only** when that insert was new. A redelivered `(entity, metric,
//! window_id)` therefore changes nothing and reports [`FlushOutcome::AlreadyApplied`].
//!
//! Schema (owned by the `migrator`, applied before rollout — see Phase 6):
//! ```sql
//! CREATE TABLE counter_windows (
//!   entity_kind text, entity_id text, metric text, window_id bigint,
//!   PRIMARY KEY (entity_kind, entity_id, metric, window_id));
//! CREATE TABLE counter_totals (
//!   entity_kind text, entity_id text, metric text, total bigint NOT NULL,
//!   PRIMARY KEY (entity_kind, entity_id, metric));
//! ```

use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::{CounterLedger, FlushOutcome};
use crate::domain::{EntityRef, Metric, WindowDelta};
use crate::error::CounterError;

/// Atomically: record the window (idempotent), and advance the total only if the
/// window was new. Returns whether the window was applied for the first time.
const FLUSH_SQL: &str = r#"
WITH ins AS (
    INSERT INTO counter_windows (entity_kind, entity_id, metric, window_id)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT DO NOTHING
    RETURNING 1
), upd AS (
    INSERT INTO counter_totals (entity_kind, entity_id, metric, total)
    SELECT $1, $2, $3, $5 FROM ins
    ON CONFLICT (entity_kind, entity_id, metric) DO UPDATE
        SET total = counter_totals.total + EXCLUDED.total
    RETURNING 1
)
SELECT EXISTS (SELECT 1 FROM ins) AS applied
"#;

const READ_TOTAL_SQL: &str = r#"
SELECT total FROM counter_totals
WHERE entity_kind = $1 AND entity_id = $2 AND metric = $3
"#;

/// Reconciliation overwrite: set the durable total to an authoritative value.
const SET_TOTAL_SQL: &str = r#"
INSERT INTO counter_totals (entity_kind, entity_id, metric, total)
VALUES ($1, $2, $3, $4)
ON CONFLICT (entity_kind, entity_id, metric) DO UPDATE SET total = EXCLUDED.total
"#;

fn flush_err(e: sqlx::Error) -> CounterError {
    CounterError::FlushFailed {
        reason: e.to_string(),
    }
}

/// Ledger reads fall back nowhere (they *are* the fallback), so a fault here is the
/// unavailable (retryable) variant.
fn read_err(_e: sqlx::Error) -> CounterError {
    CounterError::LedgerUnavailable
}

pub struct PgCounterLedger {
    tx: TransactionManager,
}

impl PgCounterLedger {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl CounterLedger for PgCounterLedger {
    async fn flush_window(&self, delta: &WindowDelta) -> Result<FlushOutcome, CounterError> {
        let applied: bool = sqlx::query_scalar(FLUSH_SQL)
            .bind(delta.entity().kind.as_str())
            .bind(delta.entity().id.as_str())
            .bind(delta.metric().as_str())
            .bind(delta.window().index() as i64)
            .bind(delta.scalar())
            .fetch_one(self.tx.pool())
            .await
            .map_err(flush_err)?;

        Ok(if applied {
            FlushOutcome::Applied
        } else {
            FlushOutcome::AlreadyApplied
        })
    }

    async fn read_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError> {
        let total: Option<i64> = sqlx::query_scalar(READ_TOTAL_SQL)
            .bind(entity.kind.as_str())
            .bind(entity.id.as_str())
            .bind(metric.as_str())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(read_err)?;
        Ok(total)
    }

    async fn set_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError> {
        sqlx::query(SET_TOTAL_SQL)
            .bind(entity.kind.as_str())
            .bind(entity.id.as_str())
            .bind(metric.as_str())
            .bind(value)
            .execute(self.tx.pool())
            .await
            .map_err(flush_err)?;
        Ok(())
    }
}
