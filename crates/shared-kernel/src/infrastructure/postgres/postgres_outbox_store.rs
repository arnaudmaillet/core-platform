use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;
use crate::domain::events::EventEnvelope;
use crate::domain::repositories::OutboxStore;
use crate::errors::Result;
use crate::infrastructure::postgres::{OutboxRow, SqlxErrorExt};

pub struct PostgresOutboxStore {
    pool: PgPool,
}

impl PostgresOutboxStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxStore for PostgresOutboxStore {
    async fn fetch_unprocessed(&self, limit: u32) -> Result<Vec<EventEnvelope>> {
        let sql = r#"
            WITH selected AS (
                SELECT id FROM outbox
                WHERE processed_at IS NULL
                  AND attempts < 5
                ORDER BY created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            SELECT 
                id, aggregate_type, aggregate_id, event_type, 
                payload, attempts, created_at 
            FROM outbox
            WHERE id IN (SELECT id FROM selected)
            ORDER BY created_at ASC
        "#;

        let rows = sqlx::query_as::<_, OutboxRow>(sql)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_domain_infra("Failed to fetch unprocessed outbox messages")?;

        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }

    async fn mark_as_processed(&self, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        sqlx::query("UPDATE outbox SET processed_at = NOW() WHERE id = ANY($1)")
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_domain_infra("Failed to mark outbox as processed")?;

        Ok(())
    }

    async fn mark_as_failed(&self, id: Uuid, error: String) -> Result<()> {
        let sql = "UPDATE outbox SET attempts = attempts + 1, last_error = $2 WHERE id = $1";

        sqlx::query(sql)
            .bind(id)    // $1
            .bind(error) // $2
            .execute(&self.pool)
            .await
            .map_domain_infra("Failed to mark outbox as failed")?;

        Ok(())
    }
}