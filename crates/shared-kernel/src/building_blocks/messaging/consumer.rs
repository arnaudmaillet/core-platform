// crates/shared-kernel/src/application/ports/message_consumer.rs

use crate::{core::Result, messaging::EventEnvelope};
use async_trait::async_trait;
use futures_util::future::BoxFuture;

// On s'assure que le type alias est bien visible
pub type EventHandler = Box<dyn Fn(EventEnvelope) -> BoxFuture<'static, Result<()>> + Send + Sync>;

#[async_trait]
pub trait EventConsumer: Send + Sync {
    async fn consume(&self, topic: &str, handler: EventHandler) -> Result<()>;
}
