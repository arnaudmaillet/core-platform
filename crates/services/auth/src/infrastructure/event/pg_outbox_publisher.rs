//! Outbox-backed [`EventPublisher`]: enqueue-to-Postgres instead of
//! publish-to-broker.
//!
//! Handlers keep the same port; what changes is the fault domain. Login's
//! event "publish" now succeeds or fails with the SAME store its session
//! writes already depend on — broker unavailability can no longer hang or
//! fail the RPC (found live: a misconfigured broker held Login at deadline),
//! and a committed session can no longer lose its compliance event (audit
//! consumes `auth.v1.events`). The [`super::outbox_relay::OutboxRelay`]
//! drains this table to the real broker in the background.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::port::EventPublisher;
use crate::domain::event::DomainEvent;
use crate::error::AuthError;

pub struct PgOutboxPublisher {
    pool: PgPool,
}

impl PgOutboxPublisher {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventPublisher for PgOutboxPublisher {
    async fn publish(&self, event: &DomainEvent) -> Result<(), AuthError> {
        let payload = serde_json::to_value(event)
            .map_err(|e| AuthError::EventPublishFailed(format!("outbox serialize: {e}")))?;

        // UUIDv7 id: time-ordered, so (created_at, id) is a stable drain order
        // and per-account Kafka partition ordering survives the indirection.
        sqlx::query(
            "INSERT INTO auth_outbox (id, event_type, payload) VALUES ($1, $2, $3)",
        )
        .bind(Uuid::now_v7())
        .bind(event.event_type())
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::EventPublishFailed(format!("outbox enqueue: {e}")))?;

        Ok(())
    }
}
