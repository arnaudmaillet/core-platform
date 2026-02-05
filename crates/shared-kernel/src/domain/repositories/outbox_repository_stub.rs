use crate::domain::events::{DomainEvent, EventEnvelope};
use crate::domain::repositories::OutboxRepository;
use crate::domain::transaction::Transaction;

// --- STUB OUTBOX ---
pub struct OutboxRepoStub;
#[async_trait::async_trait]
impl OutboxRepository for OutboxRepoStub {
    async fn save(&self, _tx: &mut dyn Transaction, _event: &dyn DomainEvent) -> crate::errors::Result<()> {
        Ok(())
    }

    async fn find_pending(&self, _limit: i32) -> crate::errors::Result<Vec<EventEnvelope>> {
        Ok(vec![])
    }
}
