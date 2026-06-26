//! Hot-path read cache for delivery resolution (Redis adapter, Phase 4). Caches the
//! asset projection so `ResolveDelivery` does not hit Postgres on every feed
//! render. Best-effort: a miss falls back to the repository, an error is treated
//! as a miss (the read path fails open).

use async_trait::async_trait;

use crate::domain::aggregate::Asset;
use crate::domain::value_object::AssetId;
use crate::error::MediaError;

#[async_trait]
pub trait DeliveryCache: Send + Sync + 'static {
    async fn get(&self, id: &AssetId) -> Result<Option<Asset>, MediaError>;

    async fn put(&self, asset: &Asset) -> Result<(), MediaError>;

    /// Drops the cached entry — called on delete, quarantine, and reprocess so a
    /// stale deliverable view can't outlive a takedown.
    async fn invalidate(&self, id: &AssetId) -> Result<(), MediaError>;
}
