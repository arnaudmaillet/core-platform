//! Outbound port for publishing lifecycle events to `media.v1.events` (Phase 4
//! Kafka adapter), keyed by `asset_id` for per-asset ordering.
//!
//! Handlers persist durably first, then publish — the durable write is the source
//! of truth, the event is the decoupling feed `post`/`profile`/`search` react to.

use async_trait::async_trait;

use crate::domain::event::DomainEvent;
use crate::error::MediaError;

#[async_trait]
pub trait EventPublisher: Send + Sync + 'static {
    async fn publish(&self, event: &DomainEvent) -> Result<(), MediaError>;
}
