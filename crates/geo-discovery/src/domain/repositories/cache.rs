// crates/geo_discovery/src/domain/repositories/map_cache_repository.rs

use crate::domain::types::{H3Tile, TileResolution};
use crate::types::{ScoredPostTile, TilePostMetadata};
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
        metadata: &TilePostMetadata,
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
    ) -> Result<Vec<ScoredPostTile>>;

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
    ) -> Result<Vec<TilePostMetadata>>;

    /// Enregistre une tuile dans l'index global pour indiquer qu'elle contient des posts actifs
    async fn track_active_tile(&self, resolution: TileResolution, tile_id: &H3Tile) -> Result<()>;

    /// Retire une tuile de l'index global (à utiliser lorsqu'elle n'a plus aucun post actif)
    async fn untrack_active_tile(&self, resolution: TileResolution, tile_id: &H3Tile)
    -> Result<()>;

    /// Récupère l'intégralité des tuiles géographiques ayant enregistré de l'activité
    async fn get_all_active_tiles(&self) -> Result<Vec<(TileResolution, H3Tile)>>;

    /// Retourne le nombre total de posts indexés dans le ZSET de popularité pour une tuile donnée
    async fn get_tile_post_count(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
    ) -> Result<usize>;
}
