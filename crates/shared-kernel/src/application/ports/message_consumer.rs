// crates/shared-kernel/src/application/ports/message_consumer.rs

use crate::core::Result;
use crate::domain::events::EventEnvelope;
use async_trait::async_trait;
use futures_util::future::BoxFuture;

// On s'assure que le type alias est bien visible
pub type MessageHandler =
    Box<dyn Fn(EventEnvelope) -> BoxFuture<'static, Result<()>> + Send + Sync>;

#[async_trait]
pub trait MessageConsumer: Send + Sync {
    async fn consume(&self, topic: &str, handler: MessageHandler) -> Result<()>;
}
