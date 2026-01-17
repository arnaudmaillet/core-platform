// crates/shared-kernel/src/persistence/postgres/outbox_repository

use sqlx::{query, Pool, Postgres};
use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::transaction::Transaction;
use crate::infrastructure::transaction::TransactionExt;
use crate::errors::Result;
use crate::infrastructure::postgres::SqlxErrorExt;

pub struct PostgresOutboxRepository {
    pool: Pool<Postgres>,
}

impl PostgresOutboxRepository {
    pub fn new(pool: Pool<Postgres>) -> Self { Self { pool } }

    /// Sauvegarde l'événement (Write)
    pub async fn append(&self, tx: &mut dyn Transaction, event: &dyn DomainEvent) -> Result<()> {
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