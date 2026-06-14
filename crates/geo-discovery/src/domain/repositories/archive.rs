// crates/geo_discovery/src/domain/repositories/map_persistence_repository.rs

use crate::entities::MapAnnotation;
use crate::types::{BucketHour, TileH3, TileResolution};
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::PostId;
use std::time::Duration;

#[async_trait]
pub trait MapAnnotationArchiveRepository: Send + Sync {
    /// Persiste un post géo-indexé avec un TTL dynamique calculé par l'application
    async fn save(&self, post: &MapAnnotation, ttl: Duration) -> Result<()>;

    /// Récupère tous les posts d'une tuile spécifique pour un segment temporel donné
    /// Utile pour les stratégies de rechargement ou d'audit
    async fn find_by_tile(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        bucket: BucketHour,
    ) -> Result<Vec<MapAnnotation>>;

    /// Supprime explicitement un post d'une tuile (ex: si l'utilisateur supprime son post)
    async fn delete(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        bucket: BucketHour,
        post_id: &PostId,
    ) -> Result<()>;
}
