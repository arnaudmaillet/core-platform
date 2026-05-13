// crates/shared-kernel/src/infrastructure/postgres/rows/postgres_idempotency_row.rs

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
pub struct IdempotencyRow {
    pub command_id: Uuid,
    pub namespace: String,
    pub processed_at: DateTime<Utc>,
}