// crates/shared-kernel/src/infrastructure/postgres/rows/postgres_outbox_row.rs


use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;
use serde_json::Value;
use crate::domain::events::EventEnvelope;

/// Struct privé à l'infrastructure pour le mapping SQLx
#[derive(FromRow)]
pub struct OutboxRow {
    id: Uuid,
    region_code: String,
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    payload: Value,
    metadata: Option<Value>,
    occurred_at: DateTime<Utc>,
}

impl From<OutboxRow> for EventEnvelope {
    fn from(row: OutboxRow) -> Self {
        Self {
            id: row.id,
            region_code: row.region_code,
            aggregate_type: row.aggregate_type,
            aggregate_id: row.aggregate_id,
            event_type: row.event_type,
            payload: row.payload,
            occurred_at: row.occurred_at,
            metadata: row.metadata,
        }
    }
}