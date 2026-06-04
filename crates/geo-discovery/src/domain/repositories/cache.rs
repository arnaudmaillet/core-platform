// crates/geo_discovery/src/domain/repositories/map_cache_repository.rs

use crate::domain::types::{H3Tile, TileResolution};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared_kernel::core::Result;
use shared_kernel::types::PostId;

#[async_trait]
pub trait MapCacheRepository: Send + Sync {
    /// Initialise ou met à jour le post dans le ZSET de popularité et le ZSET temporel d'une tuile
    async fn add_to_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        post_id: &PostId,
        initial_score: f64,
        created_at: DateTime<Utc>,
    ) -> Result<()>;

    /// Incrémente ou décrémente dynamiquement le score de viralité d'un post (ex: suite à un Like/Share via Kafka)
    async fn increment_score(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        post_id: &PostId,
        delta: f64,
    ) -> Result<()>;

    /// Récupère la liste ordonnée des IDs de posts les plus populaires dans une tuile spécifique (Pagination top-K)
    async fn get_top_posts(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        limit: usize,
    ) -> Result<Vec<PostId>>;

    /// Supprime un post spécifique des index de la tuile (Popularité + Temps)
    async fn remove_from_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        post_id: &PostId,
    ) -> Result<()>;

    /// Nettoie et expulse tous les posts plus vieux que la date charnière fournie.
    /// Retourne la liste des IDs effectivement supprimés pour permettre des cascades si nécessaire.
    async fn evict_old_posts(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<PostId>>;
}
