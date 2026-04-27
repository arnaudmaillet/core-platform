// crates/shared_kernel/src/domain/repositories/idempotency_repository.rs

use crate::domain::transaction::Transaction;
use crate::errors::Result;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    async fn exists(&self, tx: &mut dyn Transaction, command_id: &Uuid) -> Result<bool>;
    async fn save(&self, tx: &mut dyn Transaction, command_id: &Uuid) -> Result<()>;
}