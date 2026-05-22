// crates/shared-kernel/src/persistence/postgres/repositories/outbox_repository.rs

use crate::{OutboxRow, TransactionExt};
use async_trait::async_trait;
use shared_kernel::core::{Error, Result, Transaction};
use shared_kernel::messaging::{Event, EventEnvelope, OutboxRepository, OutboxStore};
use sqlx::{Pool, Postgres, QueryBuilder};
use uuid::Uuid;

pub struct PostgresOutboxRepository {
    pool: Pool<Postgres>,
}

impl PostgresOutboxRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

// 1. Implémentation du trait OutboxStore pour ton OutboxProcessor générique !
#[async_trait]
impl OutboxStore for PostgresOutboxRepository {
    async fn fetch_unprocessed(&self, limit: u32) -> Result<Vec<EventEnvelope>> {
        // 💡 Utilisation d'une transaction interne pour sécuriser le verrou 'FOR UPDATE SKIP LOCKED'
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Error::database(format!("Outbox tx begin failed: {}", e)))?;

        let sql = r#"
            SELECT id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at
            FROM outbox_events
            WHERE status = 'PENDING'
            ORDER BY occurred_at ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
        "#;

        let rows = sqlx::query_as::<_, OutboxRow>(sql)
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

        sqlx::query(
            r#"
            UPDATE outbox_events 
            SET status = 'PROCESSED', processed_at = NOW() 
            WHERE id = ANY($1)
            "#,
        )
        .bind(ids)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Outbox mark processed failed: {}", e)))?;

        Ok(())
    }

    async fn mark_as_failed(&self, id: Uuid, last_error: String) -> Result<()> {
        // Optionnel : si tu veux stocker les logs d'erreurs d'envoi Kafka dans l'outbox
        sqlx::query(
            r#"
            UPDATE outbox_events 
            SET status = 'FAILED', metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{last_error}', to_jsonb($2::text))
            WHERE id = $1
            "#
        )
        .bind(id)
        .bind(last_error)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::database(format!("Outbox mark failed stepped: {}", e)))?;

        Ok(())
    }
}

// 2. Ton trait classique pour la couche d'écriture (save_all)
#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn save_all(&self, tx: &mut dyn Transaction, events: &[&dyn Event]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let envelopes: Vec<EventEnvelope> =
            events.iter().map(|e| EventEnvelope::wrap(*e)).collect();

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO outbox_events (id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at) ",
        );

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
            .map_err(|_| Error::database("Outbox Bulk Insert"))?;

        Ok(())
    }

    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>> {
        let sql = r#"
            SELECT id, region, aggregate_type, aggregate_id, event_type, payload, metadata, occurred_at
            FROM outbox_events
            WHERE status = 'PENDING'
            ORDER BY occurred_at ASC
            LIMIT $1
        "#;

        let rows = sqlx::query_as::<_, OutboxRow>(sql)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        Ok(rows.into_iter().map(EventEnvelope::from).collect())
    }
}
