use std::time::Duration;

use fred::interfaces::LuaInterface;
use fred::interfaces::SortedSetsInterface;
use redis_storage::RedisClient;

use crate::infrastructure::cache::redis_spatial_index::HOT_TILES_KEY;

// ── Lua script ────────────────────────────────────────────────────────────────

/// Atomically evicts cold tile ZSETs and removes them from the hot_tiles tracker.
///
/// KEYS[1] = sg:geo:hot_tiles (tracking ZSET)
/// ARGV[1] = cutoff_epoch     (Unix timestamp; tiles with score ≤ this are cold)
/// ARGV[2] = batch_size       (maximum number of tiles to evict per run)
/// ARGV[3] = tile_key_prefix  ("sg:geo:tile:" — avoids hardcoding in Lua)
///
/// Returns: number of tile ZSETs evicted (integer).
///
/// Design notes:
/// - `ZRANGEBYSCORE ... LIMIT 0 N` bounds the per-run eviction to a fixed batch,
///   preventing latency spikes on Redis from large accumulated cold sets.
/// - `DEL` on the constructed tile keys is safe because the hot_tiles member is
///   stored as `{h3_index}:{resolution}` (the suffix after the prefix).
/// - The script is not cluster-safe: all tile keys must reside on the same Redis
///   node. This service assumes a standalone Redis or a single-node Redis Cluster
///   shard. Cluster compatibility would require client-side iteration.
const PRUNE_COLD_TILES_SCRIPT: &str = r#"
local hot_tiles_key = KEYS[1]
local cutoff        = ARGV[1]
local batch_size    = ARGV[2]
local prefix        = ARGV[3]

local cold = redis.call('ZRANGEBYSCORE', hot_tiles_key, '-inf', cutoff, 'LIMIT', '0', batch_size)
if #cold == 0 then
    return 0
end

for _, suffix in ipairs(cold) do
    redis.call('DEL', prefix .. suffix)
end

redis.call('ZREMRANGEBYSCORE', hot_tiles_key, '-inf', cutoff)

return #cold
"#;

const TILE_KEY_PREFIX: &str = "sg:geo:tile:";

fn fred_err(e: fred::error::Error) -> String {
    format!("Redis error: {}", e)
}

// ── TilePrunerWorker ──────────────────────────────────────────────────────────

/// Background task that evicts cold tile ZSETs from Redis on a fixed interval.
///
/// A tile is considered "cold" if it has not been queried within
/// `cold_threshold`. Cold tiles have their ZSET key deleted from Redis;
/// the next query for that tile performs a ScyllaDB cold-start read and
/// re-populates the ZSET via the PostIndexerWorker.
///
/// Run once every `interval` seconds. Each run processes at most `batch_size`
/// tiles to bound Redis latency.
pub struct TilePrunerWorker {
    client:         RedisClient,
    interval:       Duration,
    cold_threshold: Duration,
    batch_size:     usize,
}

impl TilePrunerWorker {
    pub fn new(
        client:         RedisClient,
        interval:       Duration,
        cold_threshold: Duration,
        batch_size:     usize,
    ) -> Self {
        Self { client, interval, cold_threshold, batch_size }
    }

    pub async fn run(self) {
        tracing::info!(
            interval_secs       = self.interval.as_secs(),
            cold_threshold_secs = self.cold_threshold.as_secs(),
            batch_size          = self.batch_size,
            "tile pruner worker started"
        );

        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Err(e) = self.prune_once().await {
                tracing::error!(error = %e, "tile pruner cycle failed");
            }
        }
    }

    async fn prune_once(&self) -> Result<(), String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::from_std(self.cold_threshold)
            .map_err(|e| e.to_string())?)
        .timestamp() as f64;

        let evicted: i64 = self.client.inner
            .eval(
                PRUNE_COLD_TILES_SCRIPT,
                vec![HOT_TILES_KEY.to_owned()],
                vec![
                    format!("{}", cutoff),
                    self.batch_size.to_string(),
                    TILE_KEY_PREFIX.to_owned(),
                ],
            )
            .await
            .map_err(fred_err)?;

        if evicted > 0 {
            tracing::info!(evicted_tiles = evicted, "cold tile ZSETs evicted from Redis");
        } else {
            tracing::debug!("tile pruner: no cold tiles found");
        }

        Ok(())
    }

    /// Returns the current cardinality of the hot_tiles tracking set.
    /// Useful for Prometheus gauge instrumentation.
    pub async fn hot_tile_count(&self) -> Result<i64, String> {
        self.client.inner
            .zcard::<i64, _>(HOT_TILES_KEY)
            .await
            .map_err(fred_err)
    }
}
