// crates/geo_discovery/src/infrastructure/repositories/fred_map_cache_repository.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use infra_fred::fred::clients::Pool;
use infra_fred::fred::interfaces::{HashesInterface, SetsInterface, SortedSetsInterface};
use infra_fred::fred::types::Value as RedisValue;
use prost::Message;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, PostType};
use std::str::FromStr;

use crate::domain::repositories::MapCacheRepository;
use crate::domain::types::{H3Tile, TilePostMetadata, TileResolution};
use crate::types::{PopularityScore, ScoredPostTile};
use shared_proto::geo_discovery::v1::TilePostMetadata as ProtoTilePostMetadata;

pub struct FredMapCacheRepository {
    pool: Pool,
}

impl FredMapCacheRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    fn popularity_key(&self, resolution: TileResolution, tile_id: &H3Tile) -> String {
        format!("geo:tile:{}:{}", resolution.value(), tile_id.value())
    }

    fn time_key(&self, resolution: TileResolution, tile_id: &H3Tile) -> String {
        format!("geo:tile:{}:{}:time", resolution.value(), tile_id.value())
    }

    fn post_metadata_key(&self) -> &'static str {
        "geo:post:metadata"
    }

    fn global_active_tiles_key(&self) -> &'static str {
        "geo:active_tiles"
    }
}

#[async_trait]
impl MapCacheRepository for FredMapCacheRepository {
    async fn add_to_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        metadata: &TilePostMetadata,
        initial_score: f64,
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let time_key = self.time_key(resolution, tile_id);
        let expires_at_ts = expires_at.timestamp_millis() as f64;
        let post_id_str = metadata.post_id.to_string();

        let pop_values = vec![(initial_score, post_id_str.clone())];
        let time_values = vec![(expires_at_ts, post_id_str.clone())];

        let fut_pop = self
            .pool
            .zadd::<i64, _, _>(pop_key, None, None, false, false, pop_values);
        let fut_time = self
            .pool
            .zadd::<i64, _, _>(time_key, None, None, false, false, time_values);

        let proto_message = ProtoTilePostMetadata {
            post_id: post_id_str.clone(),
            latitude: metadata.latitude,
            longitude: metadata.longitude,
            post_type: metadata.post_type.to_string(),
            thumbnail_url: metadata.thumbnail_url.clone(),
        };

        let mut buffer = Vec::with_capacity(proto_message.encoded_len());
        proto_message.encode(&mut buffer).map_err(|e| {
            Error::internal(format!("Failed to serialize metadata to Protobuf: {}", e))
        })?;

        let fut_hash = self.pool.hset::<i64, _, _>(
            self.post_metadata_key(),
            (post_id_str, RedisValue::Bytes(buffer.into())),
        );

        let (res_pop, res_time, res_hash) = tokio::join!(fut_pop, fut_time, fut_hash);

        res_pop.map_err(|e| Error::internal(format!("Redis popularity write failed: {}", e)))?;
        res_time.map_err(|e| Error::internal(format!("Redis time track write failed: {}", e)))?;
        res_hash
            .map_err(|e| Error::internal(format!("Redis metadata HASH write failed: {}", e)))?;

        Ok(())
    }

    async fn increment_score(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        post_id: &PostId,
        delta: f64,
    ) -> Result<()> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let post_id_str = post_id.to_string();

        self.pool
            .zincrby::<f64, _, _>(&pop_key, delta, post_id_str)
            .await
            .map_err(|e| Error::internal(format!("Redis ZINCRBY failed: {}", e)))?;

        Ok(())
    }

    async fn get_top_posts(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        limit: usize,
    ) -> Result<Vec<ScoredPostTile>> {
        let pop_key = self.popularity_key(resolution, tile_id);
        if limit == 0 {
            return Ok(Vec::new());
        }

        let end_idx = (limit - 1) as i64;
        let redis_pairs: Vec<(String, f64)> = self
            .pool
            .zrevrange(&pop_key, 0, end_idx, true)
            .await
            .map_err(|e| Error::internal(format!("Redis ZREVRANGE WITHSCORES failed: {}", e)))?;

        if redis_pairs.is_empty() {
            return Ok(Vec::new());
        }

        let post_ids: Vec<String> = redis_pairs.iter().map(|(id, _)| id.clone()).collect();

        let raw_buffers: Vec<Option<Vec<u8>>> = self
            .pool
            .hmget(self.post_metadata_key(), post_ids)
            .await
            .map_err(|e| Error::internal(format!("Redis HMGET metadata failed: {}", e)))?;

        let mut result_list = Vec::with_capacity(redis_pairs.len());

        for (index, (id, score)) in redis_pairs.into_iter().enumerate() {
            if let Some(Some(bytes)) = raw_buffers.get(index) {
                if let Ok(proto_meta) = ProtoTilePostMetadata::decode(bytes.as_slice()) {
                    // PARSING ROBUSTE : On valide la String infra vers l'Enum Domaine
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
        tile_id: &H3Tile,
        post_id: &PostId,
    ) -> Result<()> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let time_key = self.time_key(resolution, tile_id);
        let post_id_str = post_id.to_string();

        let fut_pop = self.pool.zrem::<i64, _, _>(&pop_key, post_id_str.clone());
        let fut_time = self.pool.zrem::<i64, _, _>(&time_key, post_id_str.clone());

        let (res_pop, res_time) = tokio::join!(fut_pop, fut_time);
        res_pop.map_err(|e| Error::internal(format!("Redis ZREM popularity failed: {}", e)))?;
        res_time.map_err(|e| Error::internal(format!("Redis ZREM time failed: {}", e)))?;

        Ok(())
    }

    async fn evict_old_posts(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<TilePostMetadata>> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let time_key = self.time_key(resolution, tile_id);
        let max_score = older_than.timestamp_millis() as f64;

        let expired_ids: Vec<String> = self
            .pool
            .zrangebyscore(&time_key, f64::MIN, max_score, false, None)
            .await
            .map_err(|e| Error::internal(format!("Redis scanning expired posts failed: {}", e)))?;

        if expired_ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_buffers: Vec<Option<Vec<u8>>> = self
            .pool
            .hmget(self.post_metadata_key(), expired_ids.clone())
            .await
            .map_err(|e| Error::internal(format!("Redis HMGET for eviction failed: {}", e)))?;

        let evicted_metadata: Vec<TilePostMetadata> = raw_buffers
            .into_iter()
            .flatten()
            .filter_map(|bytes| {
                ProtoTilePostMetadata::decode(bytes.as_slice())
                    .ok()
                    .and_then(|proto_meta| {
                        let domain_post_type = PostType::from_str(&proto_meta.post_type).ok()?;
                        Some(TilePostMetadata::new(
                            PostId::from_str(&proto_meta.post_id).ok()?,
                            proto_meta.latitude,
                            proto_meta.longitude,
                            domain_post_type,
                            proto_meta.thumbnail_url,
                        ))
                    })
            })
            .collect();

        let fut_rem_time =
            self.pool
                .zremrangebyscore::<i64, _, _, _>(&time_key, f64::MIN, max_score);
        let fut_rem_pop = self.pool.zrem::<i64, _, _>(&pop_key, expired_ids.clone());

        let fut_rem_hash = self
            .pool
            .hdel::<i64, _, _>(self.post_metadata_key(), expired_ids);

        let (res_rem_time, res_rem_pop, res_rem_hash) =
            tokio::join!(fut_rem_time, fut_rem_pop, fut_rem_hash);

        res_rem_time.map_err(|e| {
            Error::internal(format!("Redis eviction from time track failed: {}", e))
        })?;
        res_rem_pop.map_err(|e| {
            Error::internal(format!(
                "Redis eviction from popularity track failed: {}",
                e
            ))
        })?;
        res_rem_hash.map_err(|e| {
            Error::internal(format!("Redis eviction from metadata HASH failed: {}", e))
        })?;

        Ok(evicted_metadata)
    }

    async fn track_active_tile(&self, resolution: TileResolution, tile_id: &H3Tile) -> Result<()> {
        let entry = format!("{}:{}", resolution.value(), tile_id.value());
        self.pool
            .sadd::<i64, _, _>(self.global_active_tiles_key(), entry)
            .await
            .map_err(|e| Error::internal(format!("Redis SADD active tile failed: {}", e)))?;
        Ok(())
    }

    async fn untrack_active_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
    ) -> Result<()> {
        let entry = format!("{}:{}", resolution.value(), tile_id.value());
        self.pool
            .srem::<i64, _, _>(self.global_active_tiles_key(), entry)
            .await
            .map_err(|e| Error::internal(format!("Redis SREM active tile failed: {}", e)))?;
        Ok(())
    }

    async fn get_all_active_tiles(&self) -> Result<Vec<(TileResolution, H3Tile)>> {
        let raw_entries: Vec<String> = self
            .pool
            .smembers(self.global_active_tiles_key())
            .await
            .map_err(|e| Error::internal(format!("Redis SMEMBERS failed: {}", e)))?;

        let mut parsed_tiles = Vec::new();
        for entry in raw_entries {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() == 2 {
                if let Ok(res_val) = parts[0].parse::<i32>() {
                    if let Ok(resolution) = TileResolution::try_new(res_val) {
                        if let Ok(tile_id) = H3Tile::from_str(parts[1]) {
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
        tile_id: &H3Tile,
    ) -> Result<usize> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let count: usize = self
            .pool
            .zcard(&pop_key)
            .await
            .map_err(|e| Error::internal(format!("Redis ZCARD failed: {}", e)))?;
        Ok(count)
    }
}
