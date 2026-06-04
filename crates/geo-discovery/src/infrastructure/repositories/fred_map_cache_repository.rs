// crates/geo_discovery/src/infrastructure/repositories/fred_map_cache_repository.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use infra_fred::fred::clients::Pool;
use infra_fred::fred::interfaces::SortedSetsInterface;
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::PostId;
use uuid::Uuid;

use crate::domain::repositories::MapCacheRepository;
use crate::domain::types::{H3Tile, TileResolution};

pub struct FredMapCacheRepository {
    pool: Pool,
}

impl FredMapCacheRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Clé pour le ZSET de popularité / visibilité
    fn popularity_key(&self, resolution: TileResolution, tile_id: &H3Tile) -> String {
        format!("geo:tile:{}:{}", resolution.value(), tile_id.value())
    }

    /// Clé pour le ZSET temporel (Suivi de l'obsolescence des 48h)
    fn time_key(&self, resolution: TileResolution, tile_id: &H3Tile) -> String {
        format!("geo:tile:{}:{}:time", resolution.value(), tile_id.value())
    }
}

#[async_trait]
impl MapCacheRepository for FredMapCacheRepository {
    async fn add_to_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        post_id: &PostId,
        initial_score: f64,
        created_at: DateTime<Utc>,
    ) -> Result<()> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let time_key = self.time_key(resolution, tile_id);
        let post_id_str = post_id.to_string();
        let timestamp = created_at.timestamp_millis() as f64;

        let pop_values = vec![(initial_score, post_id_str.clone())];
        let time_values = vec![(timestamp, post_id_str)];

        let fut_pop = self.pool.zadd::<i64, _, _>(
            pop_key, None,  // Option<SetOptions>
            None,  // Option<Ordering>
            false, // changed
            false, // incr
            pop_values,
        );

        let fut_time = self
            .pool
            .zadd::<i64, _, _>(time_key, None, None, false, false, time_values);

        let (res_pop, res_time) = tokio::join!(fut_pop, fut_time);

        res_pop.map_err(|e| Error::internal(format!("Redis popularity write failed: {}", e)))?;
        res_time.map_err(|e| Error::internal(format!("Redis time track write failed: {}", e)))?;

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
    ) -> Result<Vec<PostId>> {
        let pop_key = self.popularity_key(resolution, tile_id);

        if limit == 0 {
            return Ok(Vec::new());
        }

        // ZREVRANGE key 0 (limit - 1) pour avoir les meilleurs scores décroissants
        let end_idx = (limit - 1) as i64;

        let raw_ids: Vec<String> = self
            .pool
            .zrevrange(&pop_key, 0, end_idx, false)
            .await
            .map_err(|e| Error::internal(format!("Redis ZREVRANGE failed: {}", e)))?;

        let post_ids = raw_ids
            .into_iter()
            .filter_map(|id_str| Uuid::parse_str(&id_str).ok())
            .map(|uuid| PostId::from_uuid(uuid))
            .collect();

        Ok(post_ids)
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
        let fut_time = self.pool.zrem::<i64, _, _>(&time_key, post_id_str);

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
    ) -> Result<Vec<PostId>> {
        let pop_key = self.popularity_key(resolution, tile_id);
        let time_key = self.time_key(resolution, tile_id);

        // Calcul du score plafond (Maintenant - 48h) en ms
        let max_score = older_than.timestamp_millis() as f64;

        // 1. On récupère d'abord les IDs qui vont être dégagés
        // zrangebyscore(key, min, max, withscores, limit)
        let expired_ids_raw: Vec<String> = self
            .pool
            .zrangebyscore(
                &time_key,
                f64::MIN,  // Fred accepte directement le f64 grâce à TryInto<ZRange>
                max_score, // Idem ici
                false,     // withscores: bool -> FALSE car on veut uniquement les membres (IDs)
                None,      // limit: Option<Limit> -> NONE car on purge tout sans limite
            )
            .await
            .map_err(|e| Error::internal(format!("Redis scanning expired posts failed: {}", e)))?;

        if expired_ids_raw.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Nettoyage effectif dans les deux sets
        // ZREMRANGEBYSCORE pour le set temporel
        let fut_rem_time =
            self.pool
                .zremrangebyscore::<i64, _, _, _>(&time_key, f64::MIN, max_score);

        // ZREM groupé pour purger le set de popularité
        let fut_rem_pop = self
            .pool
            .zrem::<i64, _, _>(&pop_key, expired_ids_raw.clone());

        let (res_rem_time, res_rem_pop) = tokio::join!(fut_rem_time, fut_rem_pop);
        res_rem_time.map_err(|e| {
            Error::internal(format!("Redis eviction from time track failed: {}", e))
        })?;
        res_rem_pop.map_err(|e| {
            Error::internal(format!(
                "Redis eviction from popularity track failed: {}",
                e
            ))
        })?;

        // 3. Mapping vers nos domaines types
        let evicted_ids = expired_ids_raw
            .into_iter()
            .filter_map(|id_str| Uuid::parse_str(&id_str).ok())
            .map(|uuid| PostId::from_uuid(uuid))
            .collect();

        Ok(evicted_ids)
    }
}
