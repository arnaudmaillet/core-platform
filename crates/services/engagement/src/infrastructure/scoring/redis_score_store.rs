use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashSet;
use fred::interfaces::{HashesInterface, KeysInterface, LuaInterface};
use fred::types::Value as FredValue;
use redis_storage::RedisClient;
use uuid::Uuid;

use crate::application::port::{PostEngagementSnapshot, ScoreStore};
use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically swaps a profile's reaction on a post.
///
/// KEYS[1] = engagement:r:{post_id}:{profile_id}  (per-profile reaction HASH)
/// KEYS[2] = engagement:scores:{post_id}           (aggregate scores HASH)
/// ARGV[1] = new_kind  (string, e.g. "heart")
/// ARGV[2] = new_weight (string, e.g. "2")
///
/// Returns: empty array if no previous reaction, or [old_kind, old_weight] if
/// a previous reaction was replaced.
const UPSERT_SCRIPT: &str = r#"
local profile_key = KEYS[1]
local scores_key  = KEYS[2]
local new_kind    = ARGV[1]
local new_weight  = tonumber(ARGV[2])

local old_kind        = redis.call('HGET', profile_key, 'kind')
local old_weight_str  = redis.call('HGET', profile_key, 'weight')
local old_weight      = old_weight_str and tonumber(old_weight_str) or 0

if old_kind then
    redis.call('HINCRBY', scores_key, old_kind, -old_weight)
end

redis.call('HSET',    profile_key, 'kind', new_kind, 'weight', tostring(new_weight))
redis.call('HINCRBY', scores_key,  new_kind, new_weight)

if old_kind then
    return {old_kind, tostring(old_weight)}
else
    return {}
end
"#;

/// Atomically removes a profile's reaction from a post.
///
/// KEYS[1] = engagement:r:{post_id}:{profile_id}
/// KEYS[2] = engagement:scores:{post_id}
///
/// Returns: empty array if no reaction existed, or [old_kind, old_weight].
const REMOVE_SCRIPT: &str = r#"
local profile_key = KEYS[1]
local scores_key  = KEYS[2]

local old_kind       = redis.call('HGET', profile_key, 'kind')
local old_weight_str = redis.call('HGET', profile_key, 'weight')
local old_weight     = old_weight_str and tonumber(old_weight_str) or 0

if not old_kind then
    return {}
end

redis.call('HINCRBY', scores_key, old_kind, -old_weight)
redis.call('DEL', profile_key)

return {old_kind, tostring(old_weight)}
"#;

/// Atomically snapshots and resets a counter key to 0.
///
/// KEYS[1] = counter key (e.g. engagement:views:{post_id})
/// Returns: the previous value as a string, or "0" if the key did not exist.
const GETSET_ZERO_SCRIPT: &str = r#"
local key = KEYS[1]
local v   = redis.call('GET', key)
if v then
    redis.call('SET', key, '0')
    return v
else
    return '0'
end
"#;

// ── Key builders ──────────────────────────────────────────────────────────────

fn profile_key(post_id: &PostId, profile_id: &ProfileId) -> String {
    format!("engagement:r:{}:{}", post_id, profile_id)
}

fn scores_key(post_id: &PostId) -> String {
    format!("engagement:scores:{}", post_id)
}

fn views_key(post_id: &PostId) -> String {
    format!("engagement:views:{}", post_id)
}

fn shares_key(post_id: &PostId) -> String {
    format!("engagement:shares:{}", post_id)
}

fn comments_key(post_id: &PostId) -> String {
    format!("engagement:comments:{}", post_id)
}

// ── DirtyPostTracker ──────────────────────────────────────────────────────────

/// Thread-safe set of post UUIDs that have pending view/share counter increments.
///
/// Populated by `incr_view`/`incr_share` in the hot path.
/// Drained atomically by `CounterFlushWorker` every flush interval.
#[derive(Clone, Default)]
pub struct DirtyPostTracker {
    inner: Arc<DashSet<Uuid>>,
}

impl DirtyPostTracker {
    pub fn new() -> Self {
        Self { inner: Arc::new(DashSet::new()) }
    }

    pub fn mark(&self, post_id: &PostId) {
        self.inner.insert(post_id.as_uuid());
    }

    /// Drains all dirty post IDs and returns them. The set is cleared atomically.
    pub fn drain_all(&self) -> Vec<Uuid> {
        self.inner.iter().map(|r| *r.key()).collect::<Vec<_>>()
            .into_iter()
            .inspect(|id| { self.inner.remove(id); })
            .collect()
    }
}

// ── RedisScoreStore ───────────────────────────────────────────────────────────

pub struct RedisScoreStore {
    client:  RedisClient,
    tracker: DirtyPostTracker,
}

impl RedisScoreStore {
    pub fn new(client: RedisClient, tracker: DirtyPostTracker) -> Self {
        Self { client, tracker }
    }

    /// Executes a Lua script and parses the `[kind, weight]` array return.
    async fn run_swap_script(
        &self,
        script: &str,
        post_id:    &PostId,
        profile_id: &ProfileId,
        args: Vec<String>,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError> {
        let keys = vec![profile_key(post_id, profile_id), scores_key(post_id)];

        let result: Vec<String> = self.client
            .inner
            .eval(script, keys, args)
            .await
            .map_err(|e| EngagementError::Redis(redis_storage::RedisStorageError::from(e)))?;

        if result.len() == 2 {
            let kind   = ReactionKind::from_redis_key(&result[0])?;
            let weight = result[1].parse::<i64>().map_err(|_| EngagementError::ScriptReturnInvalid)?;
            Ok(Some((kind, weight)))
        } else {
            Ok(None)
        }
    }

    /// Atomically gets and resets a counter key. Returns the previous value.
    pub async fn getset_zero(&self, key: &str) -> Result<i64, EngagementError> {
        let result: String = self.client
            .inner
            .eval(GETSET_ZERO_SCRIPT, vec![key.to_owned()], Vec::<String>::new())
            .await
            .map_err(|e| EngagementError::Redis(redis_storage::RedisStorageError::from(e)))?;

        result.parse::<i64>().map_err(|_| EngagementError::ScriptReturnInvalid)
    }
}

fn fred_err(e: fred::error::Error) -> EngagementError {
    EngagementError::Redis(redis_storage::RedisStorageError::from(e))
}

#[async_trait]
impl ScoreStore for RedisScoreStore {
    async fn atomic_upsert_reaction(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
        new_kind:   ReactionKind,
        new_weight: i64,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError> {
        self.run_swap_script(
            UPSERT_SCRIPT,
            post_id,
            profile_id,
            vec![new_kind.as_redis_key().to_owned(), new_weight.to_string()],
        )
        .await
    }

    async fn atomic_remove_reaction(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError> {
        self.run_swap_script(REMOVE_SCRIPT, post_id, profile_id, Vec::new()).await
    }

    async fn incr_view(&self, post_id: &PostId) -> Result<(), EngagementError> {
        let _: i64 = self.client.inner.incr(views_key(post_id)).await.map_err(fred_err)?;
        self.tracker.mark(post_id);
        Ok(())
    }

    async fn incr_share(&self, post_id: &PostId) -> Result<(), EngagementError> {
        let _: i64 = self.client.inner.incr(shares_key(post_id)).await.map_err(fred_err)?;
        self.tracker.mark(post_id);
        Ok(())
    }

    async fn incr_comment(&self, post_id: &PostId) -> Result<(), EngagementError> {
        let _: i64 = self.client.inner.incr(comments_key(post_id)).await.map_err(fred_err)?;
        Ok(())
    }

    async fn decr_comment(&self, post_id: &PostId) -> Result<(), EngagementError> {
        let _: i64 = self.client.inner.decr(comments_key(post_id)).await.map_err(fred_err)?;
        Ok(())
    }

    async fn get_snapshot(&self, post_id: &PostId) -> Result<PostEngagementSnapshot, EngagementError> {
        let (reaction_scores, views_raw, shares_raw, comments_raw) = tokio::try_join!(
            async {
                self.client
                    .inner
                    .hgetall::<std::collections::HashMap<String, i64>, _>(scores_key(post_id))
                    .await
                    .map_err(fred_err)
            },
            async {
                self.client
                    .inner
                    .get::<Option<i64>, _>(views_key(post_id))
                    .await
                    .map_err(fred_err)
            },
            async {
                self.client
                    .inner
                    .get::<Option<i64>, _>(shares_key(post_id))
                    .await
                    .map_err(fred_err)
            },
            async {
                self.client
                    .inner
                    .get::<Option<i64>, _>(comments_key(post_id))
                    .await
                    .map_err(fred_err)
            },
        )?;

        Ok(PostEngagementSnapshot {
            reaction_scores,
            view_count:    views_raw.unwrap_or(0),
            share_count:   shares_raw.unwrap_or(0),
            comment_count: comments_raw.unwrap_or(0),
        })
    }
}

// Unused but ensures FredValue is importable for future EVALSHA migration.
#[allow(dead_code)]
fn _phantom(_: FredValue) {}
