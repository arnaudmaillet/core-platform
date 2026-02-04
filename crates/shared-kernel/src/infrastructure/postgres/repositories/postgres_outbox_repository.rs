// crates/shared-kernel/src/infrastructure/postgres/repositories/postgres_outbox_repository.rs

use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;
use crate::errors::Result;
use crate::infrastructure::postgres::mappers::SqlxErrorExt;
use crate::infrastructure::postgres::transactions::TransactionExt;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, query, Row};

pub struct PostgresOutboxRepository {
    pool: Pool<Postgres>,
}

impl PostgresOutboxRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    /// Sauvegarde l'événement (Write)
    async fn save(&self, tx: &mut dyn Transaction, event: &dyn DomainEvent) -> Result<()> {
        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let envelope = EventEnvelope::wrap(event);

        query(
            r#"
            INSERT INTO outbox_events (id, region_code, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
            .bind(envelope.id)
            .bind(&envelope.region_code)
            .bind(envelope.aggregate_type)
            .bind(envelope.aggregate_id)
            .bind(envelope.event_type)
            .bind(envelope.payload)
            .bind(envelope.metadata)
            .bind(envelope.occurred_at)
            .execute(&mut **sqlx_tx)
            .await
            .map_domain_infra("Outbox")?;

        Ok(())
    }

    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>> {
        let sql = r#"
            SELECT id, region_code, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at
            FROM outbox_events
            WHERE processed_at IS NULL
            ORDER BY occurred_at ASC
            LIMIT $1
        "#;

        let rows = sqlx::query(sql)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_domain_infra("Outbox")?;

        let envelopes = rows.into_iter().map(|row| {
            EventEnvelope {
                id: row.get("id"),
                region_code: row.get("region_code"),
                aggregate_type: row.get("aggregate_type"),
                aggregate_id: row.get("aggregate_id"),
                event_type: row.get("event_type"),
                payload: row.get("payload"),
                metadata: row.get("metadata"),
                occurred_at: row.get("occurred_at"),
            }
        }).collect();

        Ok(envelopes)
    }
}
