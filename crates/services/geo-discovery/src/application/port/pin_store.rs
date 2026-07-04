use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::RadarPin;
use crate::domain::value_object::{PostId, RetentionTtl};
use crate::error::GeoDiscoveryError;

/// Port: Redis string-backed pin projection for the Radar (pan) read path.
///
/// Pins are serialised with MessagePack and stored under `sg:geo:pin:{post_id}`
/// with a TTL matching the post's retention window. They hold only the data a
/// map marker needs (id, coordinates, thumbnail), so the high-frequency pan
/// query never pays for full card hydration.
///
/// Unlike the card cache, EVERY indexed post gets a pin (the projection mirrors
/// the spatial index): the Radar path is Redis-only and fail-open, with no
/// ScyllaDB fallback, so a post absent from the pin store is simply not rendered.
#[async_trait]
pub trait PinStore: Send + Sync {
    /// Serialises and stores a pin. Overwrites any existing entry.
    async fn set(
        &self,
        pin: &RadarPin,
        ttl: RetentionTtl,
    ) -> Result<(), GeoDiscoveryError>;

    /// Bulk-fetches pins. The result vector has the same length and ordering as
    /// `post_ids`; `None` indicates a miss (pin not in Redis or expired).
    async fn mget(
        &self,
        post_ids: &[Uuid],
    ) -> Result<Vec<Option<RadarPin>>, GeoDiscoveryError>;

    /// Deletes a pin. Used when a post is soft-deleted or admin-overridden before
    /// its natural Redis TTL.
    async fn del(
        &self,
        post_id: &PostId,
    ) -> Result<(), GeoDiscoveryError>;
}
