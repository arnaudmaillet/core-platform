// crates/geo_discovery/src/domain/repositories/map_persistence_repository.rs

use crate::entities::ActiveMapPost;
use crate::types::{BucketHour, H3Tile, TileResolution};
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::PostId;
use std::time::Duration;

#[async_trait]
pub trait MapPersistenceRepository: Send + Sync {
    /// Persiste un post géo-indexé avec un TTL dynamique calculé par l'application
    async fn save(&self, post: &ActiveMapPost, ttl: Duration) -> Result<()>;

    /// Récupère tous les posts d'une tuile spécifique pour un segment temporel donné
    /// Utile pour les stratégies de rechargement ou d'audit
    async fn find_by_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        bucket: BucketHour,
    ) -> Result<Vec<ActiveMapPost>>;

    /// Supprime explicitement un post d'une tuile (ex: si l'utilisateur supprime son post)
    async fn delete(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        bucket: BucketHour,
        post_id: &PostId,
    ) -> Result<()>;
}
