use async_trait::async_trait;
use chrono::Utc;
use shared_kernel::core::Result;
use shared_kernel::types::PostId;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use geo_discovery::entities::MapAnnotation;
use geo_discovery::repositories::MapAnnotationArchiveRepository;
use geo_discovery::types::{BucketHour, TileH3, TileResolution};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PartitionKey {
    tile_resolution: i32,
    tile_id: String,
    bucket_hour: i64,
}

pub struct MapRepositoryStub {
    state: RwLock<
        HashMap<PartitionKey, HashMap<PostId, (MapAnnotation, std::time::Instant, Duration)>>,
    >,
}

impl Default for MapRepositoryStub {
    fn default() -> Self {
        Self::new()
    }
}

impl MapRepositoryStub {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
        }
    }

    pub fn count_all(&self) -> usize {
        let state = self.state.read().unwrap();
        state.values().map(|cluster| cluster.len()).sum()
    }

    pub fn clear(&self) {
        let mut state = self.state.write().unwrap();
        state.clear();
    }
}

#[async_trait]
impl MapAnnotationArchiveRepository for MapRepositoryStub {
    async fn save(&self, post: &MapAnnotation, ttl: Duration) -> Result<()> {
        let mut state = self.state.write().unwrap();

        let key = PartitionKey {
            tile_resolution: post.resolution().value(),
            tile_id: post.tile_id().value().to_string(),
            bucket_hour: post.bucket_hour().value(),
        };

        let insertion_time = std::time::Instant::now();

        let cluster = state.entry(key).or_insert_with(HashMap::new);
        cluster.insert(post.post_id(), (post.clone(), insertion_time, ttl));

        Ok(())
    }

    async fn find_by_tile(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        bucket: BucketHour,
    ) -> Result<Vec<MapAnnotation>> {
        let state = self.state.read().unwrap();

        let key = PartitionKey {
            tile_resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
            bucket_hour: bucket.value(),
        };

        let now_utc = Utc::now();

        if let Some(cluster) = state.get(&key) {
            let active_posts = cluster
                .values()
                .filter_map(|(post, inserted_at, ttl)| {
                    if inserted_at.elapsed() >= *ttl {
                        return None;
                    }

                    if post.expires_at() <= now_utc {
                        return None;
                    }

                    Some(post.clone())
                })
                .collect();

            return Ok(active_posts);
        }

        Ok(Vec::new())
    }

    async fn delete(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        bucket: BucketHour,
        post_id: &PostId,
    ) -> Result<()> {
        let mut state = self.state.write().unwrap();

        let key = PartitionKey {
            tile_resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
            bucket_hour: bucket.value(),
        };

        if let Some(cluster) = state.get_mut(&key) {
            cluster.remove(post_id);

            if cluster.is_empty() {
                state.remove(&key);
            }
        }

        Ok(())
    }
}
