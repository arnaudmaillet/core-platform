// crates/shared-kernel/src/domain/repositories/outbox_repository.rs

use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::transaction::Transaction;
use crate::core::Result;
use async_trait::async_trait;

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn save_all(&self, tx: &mut dyn Transaction, events: &[&dyn DomainEvent]) -> Result<()>;
    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>>;
}
