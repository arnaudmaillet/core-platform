// crates/shared-kernel/src/persistence/outbox_store.rs

use async_trait::async_trait;
use uuid::Uuid;
use crate::domain::events::EventEnvelope;
use crate::errors::Result;

#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// Récupère les X prochains événements à traiter
    async fn fetch_unprocessed(&self, limit: u32) -> Result<Vec<EventEnvelope>>;

    /// Marque les événements comme traités (ou les supprime)
    async fn mark_as_processed(&self, ids: &[Uuid]) -> Result<()>;

    /// En cas d'échec de publication, on incrémente 'attempts' et on log l'erreur
    async fn mark_as_failed(&self, id: Uuid, last_error: String) -> Result<()>;
}