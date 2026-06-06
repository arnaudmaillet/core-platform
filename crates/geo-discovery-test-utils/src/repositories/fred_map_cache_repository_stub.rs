use async_trait::async_trait;
use chrono::{DateTime, Utc};
use prost::Message;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, PostType};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::RwLock;

use geo_discovery::repositories::MapCacheRepository;
use geo_discovery::types::{TileH3, TilePostMetadata, TileResolution};
use geo_discovery::types::{PopularityScore, ScoredPostTile};
use shared_proto::geo_discovery::v1::TilePostMetadata as ProtoTilePostMetadata;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TileKey {
    resolution: i32,
    tile_id: String,
}

pub struct StubMapCacheRepository {
    // geo:tile:{res}:{tile_id} -> Map de PostId -> Score (f64)
    popularity_zsets: RwLock<HashMap<TileKey, HashMap<PostId, f64>>>,
    // geo:tile:{res}:{tile_id}:time -> Map de PostId -> Expiration Timestamp (f64)
    time_zsets: RwLock<HashMap<TileKey, HashMap<PostId, f64>>>,
    // geo:post:metadata -> Map globale stockant les buffers binaires Protobuf simulés
    metadata_hash: RwLock<HashMap<PostId, Vec<u8>>>,
    // geo:active_tiles -> Set stockant la String formatée "res:tile_id"
    active_tiles_set: RwLock<HashSet<String>>,
}

impl Default for StubMapCacheRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl StubMapCacheRepository {
    pub fn new() -> Self {
        Self {
            popularity_zsets: RwLock::new(HashMap::new()),
            time_zsets: RwLock::new(HashMap::new()),
            metadata_hash: RwLock::new(HashMap::new()),
            active_tiles_set: RwLock::new(HashSet::new()),
        }
    }

    pub fn get_raw_metadata(&self, post_id: &PostId) -> Option<Vec<u8>> {
        self.metadata_hash.read().unwrap().get(post_id).cloned()
    }

    pub fn clear(&self) {
        self.popularity_zsets.write().unwrap().clear();
        self.time_zsets.write().unwrap().clear();
        self.metadata_hash.write().unwrap().clear();
        self.active_tiles_set.write().unwrap().clear();
    }
}

#[async_trait]
impl MapCacheRepository for StubMapCacheRepository {
    async fn add_to_tile(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        metadata: &TilePostMetadata,
        initial_score: f64,
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };
        let post_id = metadata.post_id;

        let mut pop_guard = self.popularity_zsets.write().unwrap();
        pop_guard
            .entry(key.clone())
            .or_default()
            .insert(post_id, initial_score);

        let mut time_guard = self.time_zsets.write().unwrap();
        let expires_at_ts = expires_at.timestamp_millis() as f64;
        time_guard
            .entry(key)
            .or_default()
            .insert(post_id, expires_at_ts);

        let proto_message = ProtoTilePostMetadata {
            post_id: post_id.to_string(),
            latitude: metadata.latitude,
            longitude: metadata.longitude,
            post_type: metadata.post_type.to_string(),
            thumbnail_url: metadata.thumbnail_url.clone(),
        };

        let mut buffer = Vec::with_capacity(proto_message.encoded_len());
        proto_message.encode(&mut buffer).map_err(|e| {
            Error::internal(format!(
                "Failed to serialize metadata to Protobuf in stub: {}",
                e
            ))
        })?;

        let mut hash_guard = self.metadata_hash.write().unwrap();
        hash_guard.insert(post_id, buffer);

        Ok(())
    }

    async fn increment_score(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        post_id: &PostId,
        delta: f64,
    ) -> Result<()> {
        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };

        let mut pop_guard = self.popularity_zsets.write().unwrap();
        if let Some(zset) = pop_guard.get_mut(&key) {
            if let Some(score) = zset.get_mut(post_id) {
                *score += delta;
            }
        }
        Ok(())
    }

    async fn get_top_posts(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        limit: usize,
    ) -> Result<Vec<ScoredPostTile>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };

        let pop_guard = self.popularity_zsets.read().unwrap();
        let zset = match pop_guard.get(&key) {
            Some(z) => z,
            None => return Ok(Vec::new()),
        };

        let mut pairs: Vec<(PostId, f64)> = zset.iter().map(|(k, v)| (*k, *v)).collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs.truncate(limit);

        let hash_guard = self.metadata_hash.read().unwrap();
        let mut result_list = Vec::with_capacity(pairs.len());

        for (post_id, score) in pairs {
            // Simuler la récupération HMGET
            if let Some(bytes) = hash_guard.get(&post_id) {
                // Simuler le décodage et décapsulage binaire
                if let Ok(proto_meta) = ProtoTilePostMetadata::decode(bytes.as_slice()) {
                    if let Ok(domain_post_type) = PostType::from_str(&proto_meta.post_type) {
                        let domain_meta = TilePostMetadata::new(
                            PostId::from_str(&proto_meta.post_id)?,
                            proto_meta.latitude,
                            proto_meta.longitude,
                            domain_post_type,
                            proto_meta.thumbnail_url,
                        );

                        let popularity = PopularityScore::from_raw(score);
                        result_list.push(ScoredPostTile::new(domain_meta, popularity));
                    }
                }
            }
        }

        Ok(result_list)
    }

    async fn remove_from_tile(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        post_id: &PostId,
    ) -> Result<()> {
        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };

        if let Some(zset) = self.popularity_zsets.write().unwrap().get_mut(&key) {
            zset.remove(post_id);
        }
        if let Some(zset) = self.time_zsets.write().unwrap().get_mut(&key) {
            zset.remove(post_id);
        }
        Ok(())
    }

    async fn evict_old_posts(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<TilePostMetadata>> {
        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };

        let max_score = older_than.timestamp_millis() as f64;
        let mut expired_ids = Vec::new();

        {
            let time_guard = self.time_zsets.read().unwrap();
            if let Some(zset) = time_guard.get(&key) {
                for (post_id, score) in zset.iter() {
                    if *score <= max_score {
                        expired_ids.push(*post_id);
                    }
                }
            }
        }

        if expired_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut evicted_metadata = Vec::new();
        let mut hash_guard = self.metadata_hash.write().unwrap();
        let mut pop_guard = self.popularity_zsets.write().unwrap();
        let mut time_guard = self.time_zsets.write().unwrap();

        for post_id in &expired_ids {
            if let Some(bytes) = hash_guard.remove(post_id) {
                if let Ok(proto_meta) = ProtoTilePostMetadata::decode(bytes.as_slice()) {
                    if let Ok(domain_post_type) = PostType::from_str(&proto_meta.post_type) {
                        evicted_metadata.push(TilePostMetadata::new(
                            PostId::from_str(&proto_meta.post_id)?,
                            proto_meta.latitude,
                            proto_meta.longitude,
                            domain_post_type,
                            proto_meta.thumbnail_url,
                        ));
                    }
                }
            }

            if let Some(zset) = pop_guard.get_mut(&key) {
                zset.remove(post_id);
            }
            if let Some(zset) = time_guard.get_mut(&key) {
                zset.remove(post_id);
            }
        }

        Ok(evicted_metadata)
    }

    async fn track_active_tile(&self, resolution: TileResolution, tile_id: &TileH3) -> Result<()> {
        let entry = format!("{}:{}", resolution.value(), tile_id.value());
        self.active_tiles_set.write().unwrap().insert(entry);
        Ok(())
    }

    async fn untrack_active_tile(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
    ) -> Result<()> {
        let entry = format!("{}:{}", resolution.value(), tile_id.value());
        self.active_tiles_set.write().unwrap().remove(&entry);
        Ok(())
    }

    async fn get_all_active_tiles(&self) -> Result<Vec<(TileResolution, TileH3)>> {
        let set_guard = self.active_tiles_set.read().unwrap();
        let mut parsed_tiles = Vec::new();

        for entry in set_guard.iter() {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() == 2 {
                if let Ok(res_val) = parts[0].parse::<i32>() {
                    if let Ok(resolution) = TileResolution::try_new(res_val) {
                        if let Ok(tile_id) = TileH3::from_str(parts[1]) {
                            parsed_tiles.push((resolution, tile_id));
                        }
                    }
                }
            }
        }
        Ok(parsed_tiles)
    }

    async fn get_tile_post_count(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
    ) -> Result<usize> {
        let key = TileKey {
            resolution: resolution.value(),
            tile_id: tile_id.value().to_string(),
        };

        let pop_guard = self.popularity_zsets.read().unwrap();
        let count = pop_guard.get(&key).map(|zset| zset.len()).unwrap_or(0);
        Ok(count)
    }
}
