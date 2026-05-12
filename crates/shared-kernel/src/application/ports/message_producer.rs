// crates/shared-kernel/src/application/ports/message_producer.rs

use crate::domain::events::EventEnvelope;
use crate::core::Result;
use async_trait::async_trait;

#[async_trait]
pub trait MessageProducer: Send + Sync {
    /// Publie un événement sérialisé (Enveloppe) vers le bus de messages.
    /// Le broker utilisera `event.event_type` pour déterminer le topic.
    async fn publish(&self, event: &EventEnvelope) -> Result<()>;

    /// Publie un batch d'enveloppes.
    /// Très important pour Kafka afin de maximiser le débit (Hyperscale).
    async fn publish_batch(&self, events: &[EventEnvelope]) -> Result<()>;
}
