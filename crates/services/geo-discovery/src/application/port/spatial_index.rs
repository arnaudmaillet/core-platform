use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::value_object::{H3Index, H3Resolution, PostId, ViralityScore};
use crate::error::GeoDiscoveryError;

/// Port: Redis ZSET-backed spatial index.
///
/// Maintains one sorted set per `(h3_index, resolution)` pair. Members are
/// post UUIDs serialised as hyphenated strings; scores are virality values.
///
/// The Top-K cap is enforced atomically inside the Lua script on every write.
/// Reads are plain ZRANGEBYSCORE operations, cluster-safe via concurrent
/// per-key calls issued through fred's lock-free command queue.
#[async_trait]
pub trait SpatialIndex: Send + Sync {
    /// Inserts or updates a post in the ZSET for the given tile and resolution.
    ///
    /// Atomically caps the ZSET at `resolution.top_k_cap()` members after the
    /// write, evicting the lowest-score entry if the cap is exceeded.
    async fn upsert(
        &self,
        tile:      H3Index,
        res:       H3Resolution,
        post_id:   &PostId,
        score:     ViralityScore,
    ) -> Result<(), GeoDiscoveryError>;

    /// Updates the score for an existing member (XX — no insert).
    ///
    /// Returns `true` if the member existed and was updated, `false` if the
    /// post was not in the ZSET (e.g. evicted by Top-K or cold tile eviction).
    async fn update_score(
        &self,
        tile:    H3Index,
        res:     H3Resolution,
        post_id: &PostId,
        score:   ViralityScore,
    ) -> Result<bool, GeoDiscoveryError>;

    /// Returns all post IDs in the tile with score ≥ `min_score`.
    ///
    /// Also updates the tile's last-access timestamp in `sg:geo:hot_tiles`
    /// (fire-and-forget; failure is logged but does not propagate).
    async fn query(
        &self,
        tile:      H3Index,
        res:       H3Resolution,
        min_score: f64,
    ) -> Result<Vec<Uuid>, GeoDiscoveryError>;

    /// Updates the last-access score for a set of tiles in `sg:geo:hot_tiles`.
    ///
    /// Called after every successful viewport query. Failures are non-fatal.
    async fn touch_hot_tiles(
        &self,
        tiles: &[(H3Index, H3Resolution)],
    ) -> Result<(), GeoDiscoveryError>;
}
