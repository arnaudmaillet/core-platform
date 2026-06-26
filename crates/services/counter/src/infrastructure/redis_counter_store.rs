//! The hot counter tier over Redis (fred).
//!
//! All non-trivial commands are issued through Lua `eval` / `redis.call(...)` —
//! the same pattern `engagement` and `geo-discovery` use — so the adapter does not
//! depend on fred's typed HyperLogLog / sorted-set method signatures. Each script
//! touches a **single key**, keeping it Redis-Cluster-safe.
//!
//! Key layout (every per-entity key carries a `{kind:id}` hash tag so one entity's
//! keys share a cluster slot):
//! * `counter:c:{kind:id}`            — HASH, field per sum metric (live counter)
//! * `counter:h:{kind:id}:{metric}`   — HyperLogLog per cardinality metric
//! * `counter:t:{scope}:{key}:{metric}` — sorted set, the trending board
//!
//! Sharded hot writes need no Lua coordination: many workers issuing `HINCRBY`
//! against the same entity hash is already atomic in Redis, which *is* the stage-2
//! re-aggregation of the producer-side shard fan-out.

use async_trait::async_trait;
use fred::interfaces::LuaInterface;
use redis_storage::RedisClient;

use crate::application::port::CounterStore;
use crate::domain::{
    Aggregation, CountSnapshot, CounterValue, EntityId, EntityKind, EntityRef, Metric, TrendingItem,
    TrendingScope, WindowDelta,
};
use crate::error::CounterError;

const HINCRBY: &str = "return redis.call('HINCRBY', KEYS[1], ARGV[1], ARGV[2])";
const HSET: &str = "return redis.call('HSET', KEYS[1], ARGV[1], ARGV[2])";
const ZINCRBY: &str = "return redis.call('ZINCRBY', KEYS[1], ARGV[1], ARGV[2])";
const PFADD: &str = "return redis.call('PFADD', KEYS[1], unpack(ARGV))";
const HMGET: &str = "return redis.call('HMGET', KEYS[1], unpack(ARGV))";
const PFCOUNT: &str = "return redis.call('PFCOUNT', KEYS[1])";
const ZREVRANGE: &str = "return redis.call('ZREVRANGE', KEYS[1], 0, ARGV[1], 'WITHSCORES')";

fn hash_key(e: &EntityRef) -> String {
    format!("counter:c:{{{}:{}}}", e.kind.as_str(), e.id.as_str())
}

fn hll_key(e: &EntityRef, metric: Metric) -> String {
    format!(
        "counter:h:{{{}:{}}}:{}",
        e.kind.as_str(),
        e.id.as_str(),
        metric.as_str()
    )
}

fn trend_key(scope: TrendingScope, scope_key: Option<&str>, metric: Metric) -> String {
    format!(
        "counter:t:{}:{}:{}",
        scope.as_str(),
        scope_key.unwrap_or("_"),
        metric.as_str()
    )
}

fn member(e: &EntityRef) -> String {
    format!("{}:{}", e.kind.as_str(), e.id.as_str())
}

fn parse_member(s: &str) -> Option<EntityRef> {
    let (kind, id) = s.split_once(':')?;
    Some(EntityRef::new(
        EntityKind::try_from_str(kind).ok()?,
        EntityId::new(id).ok()?,
    ))
}

fn write_err(e: fred::error::Error) -> CounterError {
    CounterError::CacheWriteFailed {
        reason: e.to_string(),
    }
}

/// Hot-tier reads degrade to the warm ledger, so a fred error on the read path is
/// reported as the unavailable (retryable) variant that drives fail-open.
fn read_err(_e: fred::error::Error) -> CounterError {
    CounterError::HotStoreUnavailable
}

pub struct RedisCounterStore {
    client: RedisClient,
}

impl RedisCounterStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CounterStore for RedisCounterStore {
    async fn apply_delta(&self, delta: &WindowDelta) -> Result<(), CounterError> {
        let metric = delta.metric();
        match metric.aggregation() {
            Aggregation::Sum => {
                let _: i64 = self
                    .client
                    .inner
                    .eval(
                        HINCRBY,
                        vec![hash_key(delta.entity())],
                        vec![metric.as_str().to_owned(), delta.sum.to_string()],
                    )
                    .await
                    .map_err(write_err)?;
                // Feed the global trending board (scoped boards need context a bare
                // delta lacks, so only GLOBAL is fed here).
                let _: f64 = self
                    .client
                    .inner
                    .eval(
                        ZINCRBY,
                        vec![trend_key(TrendingScope::Global, None, metric)],
                        vec![delta.sum.to_string(), member(delta.entity())],
                    )
                    .await
                    .map_err(write_err)?;
            }
            Aggregation::Cardinality => {
                if delta.unique_members.is_empty() {
                    return Ok(());
                }
                let members: Vec<String> = delta
                    .unique_members
                    .iter()
                    .map(|m| m.as_str().to_owned())
                    .collect();
                let _: i64 = self
                    .client
                    .inner
                    .eval(PFADD, vec![hll_key(delta.entity(), metric)], members)
                    .await
                    .map_err(write_err)?;
            }
        }
        Ok(())
    }

    async fn read(
        &self,
        entities: &[EntityRef],
        metrics: &[Metric],
    ) -> Result<Vec<CountSnapshot>, CounterError> {
        let sum_metrics: Vec<Metric> = metrics
            .iter()
            .copied()
            .filter(|m| m.aggregation() == Aggregation::Sum)
            .collect();
        let card_metrics: Vec<Metric> = metrics
            .iter()
            .copied()
            .filter(|m| m.aggregation() == Aggregation::Cardinality)
            .collect();

        let mut out = Vec::with_capacity(entities.len());
        for entity in entities {
            let mut values = Vec::new();

            if !sum_metrics.is_empty() {
                let fields: Vec<String> =
                    sum_metrics.iter().map(|m| m.as_str().to_owned()).collect();
                let raw: Vec<Option<String>> = self
                    .client
                    .inner
                    .eval(HMGET, vec![hash_key(entity)], fields)
                    .await
                    .map_err(read_err)?;
                for (metric, cell) in sum_metrics.iter().zip(raw) {
                    if let Some(v) = cell.and_then(|s| s.parse::<i64>().ok()) {
                        values.push(CounterValue::new(*metric, v));
                    }
                }
            }

            for &metric in &card_metrics {
                let count: i64 = self
                    .client
                    .inner
                    .eval(PFCOUNT, vec![hll_key(entity, metric)], Vec::<String>::new())
                    .await
                    .map_err(read_err)?;
                if count > 0 {
                    values.push(CounterValue::new(metric, count));
                }
            }

            out.push(CountSnapshot::new(entity.clone(), values));
        }
        Ok(out)
    }

    async fn top_k(
        &self,
        scope: TrendingScope,
        scope_key: Option<&str>,
        metric: Metric,
        limit: usize,
    ) -> Result<Vec<TrendingItem>, CounterError> {
        let stop = (limit as i64) - 1;
        let flat: Vec<String> = self
            .client
            .inner
            .eval(
                ZREVRANGE,
                vec![trend_key(scope, scope_key, metric)],
                vec![stop.to_string()],
            )
            .await
            .map_err(read_err)?;

        let items = flat
            .chunks_exact(2)
            .enumerate()
            .filter_map(|(rank, pair)| {
                let entity = parse_member(&pair[0])?;
                let score = pair[1].parse::<f64>().ok()? as i64;
                Some(TrendingItem {
                    entity,
                    score,
                    rank: rank as u32,
                })
            })
            .collect();
        Ok(items)
    }

    async fn overwrite(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError> {
        // Set, not increment — reconciliation healing the live counter to truth.
        let _: i64 = self
            .client
            .inner
            .eval(
                HSET,
                vec![hash_key(entity)],
                vec![metric.as_str().to_owned(), value.to_string()],
            )
            .await
            .map_err(write_err)?;
        Ok(())
    }
}
