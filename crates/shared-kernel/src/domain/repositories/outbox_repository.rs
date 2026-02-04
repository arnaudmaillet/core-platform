// crates/shared-kernel/src/domain/repositories/outbox_repository.rs

use std::todo;
use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::transaction::Transaction;
use crate::errors::Result;
use async_trait::async_trait;

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    /// Sauvegarde un événement dans la table outbox au sein d'une transaction existante.
    async fn save(&self, tx: &mut dyn Transaction, event: &dyn DomainEvent) -> Result<()>;
    async fn find_pending(&self, limit: i32) -> Result<Vec<EventEnvelope>>;
}
