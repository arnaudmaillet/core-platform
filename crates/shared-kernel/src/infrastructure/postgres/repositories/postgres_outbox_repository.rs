// crates/shared-kernel/src/persistence/postgres/outbox_repository

use async_trait::async_trait;
use sqlx::{query, Pool, Postgres};
use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;
use crate::infrastructure::postgres::mappers::SqlxErrorExt;
use crate::infrastructure::postgres::transactions::TransactionExt;
use crate::errors::Result;


pub struct PostgresOutboxRepository {
    pool: Pool<Postgres>,
}

impl PostgresOutboxRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository{
    /// Sauvegarde l'événement (Write)
    async fn save(&self, tx: &mut dyn Transaction, event: &dyn DomainEvent) -> Result<()> {
        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let envelope = EventEnvelope::wrap(event);

        query(
            r#"
            INSERT INTO outbox_events (id, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
            .bind(envelope.id)
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
}