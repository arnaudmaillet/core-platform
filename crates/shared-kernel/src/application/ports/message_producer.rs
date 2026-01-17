// crates/shared-kernel/src/application/ports/message_producer.rs

use async_trait::async_trait;
use crate::domain::events::EventEnvelope;
use crate::errors::AppResult;

#[async_trait]
pub trait MessageProducer: Send + Sync {
    /// Publie un événement sérialisé (Enveloppe) vers le bus de messages.
    /// Le broker utilisera `event.event_type` pour déterminer le topic.
    async fn publish(&self, event: &EventEnvelope) -> AppResult<()>;

    /// Publie un batch d'enveloppes.
    /// Très important pour Kafka afin de maximiser le débit (Hyperscale).
    async fn publish_batch(&self, events: &[EventEnvelope]) -> AppResult<()>;
}