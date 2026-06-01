// crates/shared-kernel/src/persistence/postgres/repositories/outbox_repository.rs

use crate::{OutboxRow, TransactionExt};
use async_trait::async_trait;
use shared_kernel::core::{Error, Result, Transaction};
use shared_kernel::messaging::{Event, EventEnvelope, OutboxRepository, OutboxStore};
use shared_kernel::types::Region;
use sqlx::{Pool, Postgres, QueryBuilder};
use uuid::Uuid;

const FETCH_UNPROCESSED_QUERY: &str = r#"
    SELECT id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at
    FROM outbox_events
    WHERE status = 'PENDING'
    ORDER BY occurred_at ASC
    LIMIT $1
    FOR UPDATE SKIP LOCKED
"#;

const MARK_AS_PROCESSED_QUERY: &str = r#"
    UPDATE outbox_events 
    SET status = 'PROCESSED', processed_at = NOW() 
    WHERE id = ANY($1)
"#;

const MARK_AS_FAILED_QUERY: &str = r#"
    UPDATE outbox_events 
    SET status = 'FAILED', metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{last_error}', to_jsonb($2::text))
    WHERE id = $1
"#;

const BULK_INSERT_PREFIX: &str = r#"
    INSERT INTO outbox_events (id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at) 
"#;

const FIND_PENDING_QUERY: &str = r#"
    SELECT id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at
    FROM outbox_events
    WHERE status = 'PENDING'
    ORDER BY occurred_at ASC
    LIMIT $1
"#;

pub struct PostgresOutboxRepository {
    pool: Pool<Postgres>,
}

impl PostgresOutboxRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxStore for PostgresOutboxRepository {
    async fn fetch_unprocessed(&self, limit: u32) -> Result<Vec<EventEnvelope>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::database(format!("Outbox tx begin failed: {}", e)))?;

        let rows = sqlx::query_as::<_, OutboxRow>(FETCH_UNPROCESSED_QUERY)
            .bind(limit as i32)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| Error::database(format!("Outbox fetch pending failed: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| Error::database(format!("Outbox tx commit failed: {}", e)))?;

        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }

    async fn mark_as_processed(&self, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        sqlx::query(MARK_AS_PROCESSED_QUERY)
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::database(format!("Outbox mark processed failed: {}", e)))?;

        Ok(())
    }

    async fn mark_as_failed(&self, id: Uuid, last_error: String) -> Result<()> {
        sqlx::query(MARK_AS_FAILED_QUERY)
            .bind(id)
            .bind(last_error)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::database(format!("Outbox mark failed stepped: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn save_all(
        &self,
        region: Region,
        tx: &mut dyn Transaction,
        events: &[&dyn Event],
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let envelopes: Vec<EventEnvelope> = events
            .iter()
            .map(|e| EventEnvelope::wrap(*e, region))
            .collect();

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(BULK_INSERT_PREFIX);

        query_builder.push_values(envelopes, |mut b, env| {
            b.push_bind(env.id)
                .push_bind(env.region)
                .push_bind(env.aggregate_type)
                .push_bind(env.aggregate_id)
                .push_bind(env.event_type)
                .push_bind(env.payload)
                .push_bind(env.metadata)
                .push_bind(env.occurred_at);
        });

        query_builder
            .build()
            .execute(&mut **sqlx_tx)
            .await
            .map_err(|e| Error::database(format!("Outbox Bulk Insert failed: {}", e)))?;

        Ok(())
    }

    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query_as::<_, OutboxRow>(FIND_PENDING_QUERY)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::database(format!("Outbox find pending failed: {}", e)))?;

        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }
}
