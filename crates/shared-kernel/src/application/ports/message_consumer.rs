// crates/shared-kernel/src/application/ports/message_consumer.rs

use async_trait::async_trait;
use futures_util::future::BoxFuture;
use crate::domain::events::EventEnvelope;
use crate::errors::AppResult;

// On s'assure que le type alias est bien visible
pub type MessageHandler = Box<dyn Fn(EventEnvelope) -> BoxFuture<'static, AppResult<()>> + Send + Sync>;

#[async_trait]
pub trait MessageConsumer: Send + Sync {
    async fn consume(&self, topic: &str, handler: MessageHandler) -> AppResult<()>;
}