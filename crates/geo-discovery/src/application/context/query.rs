// crates/geo_discovery/src/application/context/query.rs

use chrono::{Duration, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use h3o::Resolution;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::Region;
use std::sync::Arc;
use tokio::sync::{Semaphore, mpsc};

use crate::context::GeoDiscoveryAppContext;
use crate::handlers::HydrateTileCacheCommand;
use crate::repositories::MapPersistenceRepository;
use crate::types::{BucketHour, H3Tile, TilePostMetadata, TileResolution};
use crate::types::{MapViewport, PopularityScore, ScoredPostTile};
use shared_proto::geo_discovery::v1::{LatLng, MapPostPin};

const MAX_CONCURRENT_TILE_READS: usize = 16;

pub struct GeoDiscoveryQueryContext {
    app_ctx: GeoDiscoveryAppContext,
    region: Region,
    hydration_sender: mpsc::Sender<HydrateTileCacheCommand>,
}

impl Clone for GeoDiscoveryQueryContext {
    fn clone(&self) -> Self {
        Self {
            app_ctx: self.app_ctx.clone(),
            region: self.region,
            hydration_sender: self.hydration_sender.clone(),
        }
    }
}

impl GeoDiscoveryQueryContext {
    pub fn new(
        app_ctx: GeoDiscoveryAppContext,
        region: Region,
        hydration_sender: mpsc::Sender<HydrateTileCacheCommand>,
    ) -> Self {
        Self {
            app_ctx,
            region,
            hydration_sender,
        }
    }

    pub async fn get_map_pins(
        &self,
        viewport: MapViewport,
        resolution: TileResolution,
        limit_per_tile: usize,
    ) -> Result<Vec<MapPostPin>> {
        let h3_resolution = Resolution::try_from(resolution.value() as u8)
            .map_err(|_| Error::validation("h3_resolution", "Invalid H3 resolution mapping"))?;

        let visible_tiles = viewport.get_intersecting_tiles(h3_resolution)?;
        if visible_tiles.is_empty() {
            return Ok(Vec::new());
        }

        let cache_repo = self.app_ctx.cache_repo();
        let persistence_repo = self.app_ctx.persistence_repo();

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TILE_READS));
        let mut workers = FuturesUnordered::new();

        for tile in visible_tiles {
            let repo = cache_repo.clone();
            let p_repo = persistence_repo.clone();
            let h_sender = self.hydration_sender.clone();
            let t = tile.clone();
            let res_kv = resolution;
            let sem_permit = semaphore.clone();

            workers.push(async move {
                let _permit = sem_permit.acquire().await.map_err(|e| {
                    Error::internal(format!("Semaphore acquisition failure: {}", e))
                })?;

                let res: Result<(H3Tile, Vec<ScoredPostTile>)> =
                    match repo.get_top_posts(res_kv, &t, limit_per_tile).await {
                        // Cache Hit
                        Ok(scored_posts) if !scored_posts.is_empty() => Ok((t, scored_posts)),
                        // Cache Miss
                        _ => {
                            let fallback_data =
                                Self::execute_pure_scylla_read(&p_repo, res_kv, &t, limit_per_tile)
                                    .await?;

                            let _ =
                                h_sender.try_send(HydrateTileCacheCommand::new(res_kv, t.clone()));

                            let fallback_scored = fallback_data
                                .into_iter()
                                .map(|meta| ScoredPostTile::new(meta, PopularityScore::default()))
                                .collect();

                            Ok((t, fallback_scored))
                        }
                    };
                res
            });
        }

        let mut all_pins = Vec::new();
        let now_proto = prost_types::Timestamp::from(std::time::SystemTime::now());

        while let Some(result) = workers.next().await {
            if let Ok((_tile, scored_posts)) = result {
                for item in scored_posts {
                    all_pins.push(MapPostPin {
                        post_id: item.metadata.post_id.to_string(),
                        location: Some(LatLng {
                            latitude: item.metadata.latitude,
                            longitude: item.metadata.longitude,
                        }),
                        media_type: item.metadata.post_type.to_string(),
                        thumbnail_url: item.metadata.thumbnail_url.unwrap_or_default(),
                        popularity_score: item.score.value(),
                        created_at: Some(now_proto.clone()),
                    });
                }
            }
        }

        Ok(all_pins)
    }

    async fn execute_pure_scylla_read(
        persistence_repo: &Arc<dyn MapPersistenceRepository>,
        resolution: TileResolution,
        tile_id: &H3Tile,
        limit: usize,
    ) -> Result<Vec<TilePostMetadata>> {
        let now = Utc::now();
        let mut active_posts = Vec::new();

        for hours_ago in &[0, 24] {
            let target_time = now - Duration::hours(*hours_ago);
            let bucket = BucketHour::from_timestamp(target_time.timestamp_millis());

            if let Ok(mut posts) = persistence_repo
                .find_by_tile(resolution, tile_id, bucket)
                .await
            {
                active_posts.append(&mut posts);
            }
        }

        active_posts.truncate(limit);

        let domain_metadata_list = active_posts
            .into_iter()
            .map(|post| {
                TilePostMetadata::new(
                    post.post_id(),
                    post.location().lat(),
                    post.location().lon(),
                    post.post_type(),
                    post.thumbnail_url().map(|url| url.to_string()),
                )
            })
            .collect();

        Ok(domain_metadata_list)
    }
}
