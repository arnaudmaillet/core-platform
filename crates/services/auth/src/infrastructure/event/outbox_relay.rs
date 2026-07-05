//! Drains the `auth_outbox` table to the real event sink (Kafka in prod).
//!
//! The sink is the same [`EventPublisher`] port the handlers used to call
//! directly, so every envelope/key/header decision stays in one place
//! ([`super::kafka_event_publisher`]) — the relay only owns durability and
//! ordering. Rows are deleted on successful publish; on a mid-batch failure
//! the successfully published prefix is deleted and the remainder is retried
//! next tick (at-least-once delivery — consumers are idempotent by fleet
//! contract).
//!
//! Multi-replica safe: the batch is claimed with `FOR UPDATE SKIP LOCKED`,
//! so concurrent relays (auth runs 2 replicas) never double-publish a row
//! that another replica is mid-flight on.

use std::sync::Arc;
use std::time::Duration;

use sqlx::{PgPool, Row as _};
use uuid::Uuid;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::AuthError;

const BATCH: i64 = 64;

#[derive(Clone)]
pub struct OutboxRelay {
    pool: PgPool,
    sink: Arc<dyn EventPublisher>,
}

impl OutboxRelay {
    pub fn new(pool: PgPool, sink: Arc<dyn EventPublisher>) -> Self {
        Self { pool, sink }
    }

    /// Claims one batch, publishes sequentially (ordering!), deletes the
    /// published prefix. Returns how many events were published.
    pub async fn tick(&self) -> Result<usize, AuthError> {
        let db = |e: sqlx::Error| AuthError::EventPublishFailed(format!("outbox drain: {e}"));

        let mut tx = self.pool.begin().await.map_err(db)?;

        let rows = sqlx::query(
            "SELECT id, payload FROM auth_outbox \
             ORDER BY created_at, id LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(BATCH)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;

        let mut published: Vec<Uuid> = Vec::with_capacity(rows.len());
        let mut publish_failure: Option<AuthError> = None;

        for row in &rows {
            let id: Uuid = row.get("id");
            let payload: serde_json::Value = row.get("payload");
            let event: DomainEvent = match serde_json::from_value(payload) {
                Ok(event) => event,
                Err(e) => {
                    // A row that cannot deserialize will never succeed: fail
                    // loudly and leave it in place for operator inspection
                    // rather than silently discarding compliance evidence.
                    publish_failure = Some(AuthError::EventPublishFailed(format!(
                        "outbox row {id} undeserializable: {e}"
                    )));
                    break;
                }
            };
            match self.sink.publish(&event).await {
                Ok(()) => published.push(id),
                Err(e) => {
                    publish_failure = Some(e);
                    break;
                }
            }
        }

        if !published.is_empty() {
            sqlx::query("DELETE FROM auth_outbox WHERE id = ANY($1)")
                .bind(&published)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
        tx.commit().await.map_err(db)?;

        match publish_failure {
            // Partial progress is progress: report the failure only when the
            // whole batch stalled, so a single poison pill still surfaces.
            Some(e) if published.is_empty() => Err(e),
            _ => Ok(published.len()),
        }
    }

    /// Runs forever; spawned by the runtime adapter next to the gRPC server.
    pub async fn run(self, interval: Duration) {
        let mut timer = tokio::time::interval(interval);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            timer.tick().await;
            match self.tick().await {
                Ok(0) => {}
                Ok(n) => tracing::debug!(published = n, "auth.outbox drained"),
                Err(error) => tracing::warn!(%error, "auth.outbox relay tick failed; will retry"),
            }
        }
    }
}
