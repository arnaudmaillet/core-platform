// crates/shared-kernel/src/persistence/postgres/rows/outbox_row.rs

use crate::messaging::EventEnvelope;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

/// Struct privé à l'infrastructure pour le mapping SQLx
#[derive(FromRow)]
pub struct OutboxRow {
    pub id: Uuid,
    pub region: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload: Value,
    pub metadata: Option<Value>,
    pub occurred_at: DateTime<Utc>,
}

impl From<OutboxRow> for EventEnvelope {
    fn from(row: OutboxRow) -> Self {
        Self {
            id: row.id,
            region: row.region,
            aggregate_type: row.aggregate_type,
            aggregate_id: row.aggregate_id,
            event_type: row.event_type,
            payload: row.payload,
            occurred_at: row.occurred_at,
            metadata: row.metadata,
        }
    }
}
