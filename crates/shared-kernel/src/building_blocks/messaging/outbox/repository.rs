// crates/shared-kernel/src/domain/repositories/outbox_repository.rs

use crate::core::{Result, Transaction};
use crate::messaging::{Event, EventEnvelope};
use async_trait::async_trait;

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn save_all(&self, tx: &mut dyn Transaction, events: &[&dyn Event]) -> Result<()>;
    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>>;
}
