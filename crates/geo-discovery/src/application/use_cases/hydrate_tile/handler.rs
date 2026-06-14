// crates/geo_discovery/src/application/commands/hydrate_tile_handler.rs

use chrono::{Duration, Utc};
use shared_kernel::core::Result;
use std::sync::Arc;

use crate::repositories::{MapAnnotationDiscoveryRepository, MapAnnotationArchiveRepository};
use crate::types::{BucketHour, PopularityScore, TilePostMetadata};
use crate::use_cases::HydrateTileCacheCommand;

pub struct HydrateTileCacheHandler {
    cache_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
    persistence_repo: Arc<dyn MapAnnotationArchiveRepository>,
    max_posts_per_tile: usize,
}

impl HydrateTileCacheHandler {
    pub fn new(
        cache_repo: Arc<dyn MapAnnotationDiscoveryRepository>,
        persistence_repo: Arc<dyn MapAnnotationArchiveRepository>,
        max_posts_per_tile: usize,
    ) -> Self {
        Self {
            cache_repo,
            persistence_repo,
            max_posts_per_tile,
        }
    }

    pub async fn handle(&self, command: HydrateTileCacheCommand) -> Result<()> {
        let now = Utc::now();
        let mut active_posts = Vec::new();

        for hours_ago in &[0, 24] {
            let target_time = now - Duration::hours(*hours_ago);
            let bucket = BucketHour::from_timestamp(target_time.timestamp_millis());

            if let Ok(mut posts) = self
                .persistence_repo
                .find_by_tile(command.resolution, &command.tile_id, bucket)
                .await
            {
                active_posts.append(&mut posts);
            }
        }

        if active_posts.is_empty() {
            return Ok(());
        }

        active_posts.truncate(self.max_posts_per_tile);

        for post in active_posts {
            let expires_at = post.expires_at();

            if expires_at <= now {
                continue;
            }

            let metadata = TilePostMetadata::new(
                post.post_id(),
                post.location().lat(),
                post.location().lon(),
                post.post_type(),
                post.thumbnail_url().map(|url| url.to_string()),
            );

            let _ = self
                .cache_repo
                .add_to_tile(
                    command.resolution,
                    &command.tile_id,
                    &metadata,
                    PopularityScore::from_raw(0.0),
                    expires_at,
                )
                .await;
        }

        let _ = self
            .cache_repo
            .track_active_tile(command.resolution, &command.tile_id)
            .await;

        Ok(())
    }
}
