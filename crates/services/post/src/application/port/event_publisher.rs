use async_trait::async_trait;
use crate::{domain::event::DomainEvent, error::PostError};

#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), PostError>;
}
