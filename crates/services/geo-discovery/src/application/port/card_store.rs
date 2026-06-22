use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::MapPostCard;
use crate::domain::value_object::{PostId, RetentionTtl};
use crate::error::GeoDiscoveryError;

/// Port: Redis string-backed card cache.
///
/// Cards are serialised with MessagePack (≈45 % smaller than JSON) and stored
/// under `sg:geo:card:{post_id}` with a TTL matching the post's retention
/// window. Reads use MGET for a single network round-trip.
///
/// Only posts with `virality_score ≥ card_cache_threshold` are written to this
/// cache; low-score posts are served exclusively from ScyllaDB on cache miss.
#[async_trait]
pub trait CardStore: Send + Sync {
    /// Serialises and stores a card. Overwrites any existing entry.
    async fn set(
        &self,
        card: &MapPostCard,
        ttl:  RetentionTtl,
    ) -> Result<(), GeoDiscoveryError>;

    /// Bulk-fetches cards in a single MGET round-trip.
    ///
    /// The result vector has the same length and ordering as `post_ids`.
    /// `None` indicates a cache miss (post not in Redis or card expired).
    async fn mget(
        &self,
        post_ids: &[Uuid],
    ) -> Result<Vec<Option<MapPostCard>>, GeoDiscoveryError>;

    /// Deletes a card. Used when a post is soft-deleted or its retention expires
    /// before the natural Redis TTL (edge case: admin overrides).
    async fn del(
        &self,
        post_id: &PostId,
    ) -> Result<(), GeoDiscoveryError>;
}
