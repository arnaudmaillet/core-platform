// crates/shared-kernel/src/application/idempotency/repository.rs

use crate::core::{Result, Transaction};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    async fn exists(
        &self,
        tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<bool>;
    async fn save(&self, tx: Option<&mut (dyn Transaction + '_)>, command_id: &Uuid) -> Result<()>;
}
