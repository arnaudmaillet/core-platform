// crates/shared-kernel/src/domain/repositories/outbox_repository.rs

use async_trait::async_trait;
use crate::domain::events::DomainEvent;
use crate::domain::transaction::Transaction;
use crate::errors::Result;

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    /// Sauvegarde un événement dans la table outbox au sein d'une transaction existante.
    async fn save(&self, tx: &mut dyn Transaction, event: &dyn DomainEvent) -> Result<()>;
}