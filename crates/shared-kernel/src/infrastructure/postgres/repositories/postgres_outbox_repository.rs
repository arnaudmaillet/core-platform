// crates/shared-kernel/src/infrastructure/postgres/repositories/postgres_outbox_repository.rs

use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;
use crate::errors::Result;
use crate::infrastructure::postgres::mappers::SqlxErrorExt;
use crate::infrastructure::postgres::transactions::TransactionExt;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, QueryBuilder, Row};

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
    async fn save_all(&self, tx: &mut dyn Transaction, events: &[&dyn DomainEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let envelopes: Vec<EventEnvelope> =
            events.iter().map(|e| EventEnvelope::wrap(*e)).collect();

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO outbox_events (id, region_code, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at) ",
        );

        query_builder.push_values(envelopes, |mut b, env| {
            b.push_bind(env.id)
                .push_bind(env.region_code)
                .push_bind(env.aggregate_type)
                .push_bind(env.aggregate_id)
                .push_bind(env.event_type)
                .push_bind(env.payload)
                .push_bind(env.metadata)
                .push_bind(env.occurred_at);
        });

        let query = query_builder.build();
        query
            .execute(&mut **sqlx_tx)
            .await
            .map_domain_infra("Outbox Bulk Insert")?;

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

        let envelopes = rows
            .into_iter()
            .map(|row| EventEnvelope {
                id: row.get("id"),
                region_code: row.get("region_code"),
                aggregate_type: row.get("aggregate_type"),
                aggregate_id: row.get("aggregate_id"),
                event_type: row.get("event_type"),
                payload: row.get("payload"),
                metadata: row.get("metadata"),
                occurred_at: row.get("occurred_at"),
            })
            .collect();

        Ok(envelopes)
    }
}
