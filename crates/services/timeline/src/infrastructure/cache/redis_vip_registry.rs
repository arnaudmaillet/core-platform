use async_trait::async_trait;
use fred::interfaces::{KeysInterface, LuaInterface};
use redis_storage::RedisClient;

use crate::application::port::VipRegistry;
use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, PostId};
use crate::error::TimelineError;

// ── Key builder ───────────────────────────────────────────────────────────────

fn vip_key(author_id: &AuthorId) -> String {
    format!("timeline:vip:{}", author_id)
}

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically adds a post to a VIP registry ZSET, enforces the cap,
/// and refreshes the key TTL.
///
/// KEYS[1] = timeline:vip:{author_id}
/// ARGV[1] = score    (published_at_ms as integer string)
/// ARGV[2] = member   (post_id UUID string — author_id is the key suffix)
/// ARGV[3] = cap      (integer string, e.g. "200")
/// ARGV[4] = ttl_secs (integer string, e.g. "604800")
///
/// Returns: ZCARD after the operation.
const VIP_ZADD_CAP_TTL_SCRIPT: &str = r#"
local key      = KEYS[1]
local score    = ARGV[1]
local member   = ARGV[2]
local cap      = tonumber(ARGV[3])
local ttl_secs = tonumber(ARGV[4])

redis.call('ZADD', key, score, member)

local card = redis.call('ZCARD', key)
if card > cap then
    redis.call('ZREMRANGEBYRANK', key, 0, card - cap - 1)
end

redis.call('EXPIRE', key, ttl_secs)

return redis.call('ZCARD', key)
"#;

/// Returns at most `limit` most-recent posts from a VIP registry, newest-first.
///
/// KEYS[1] = timeline:vip:{author_id}
/// ARGV[1] = max_score (integer string, inclusive upper bound)
/// ARGV[2] = limit     (integer string)
///
/// Returns: interleaved [member1, score1, member2, score2, ...].
const VIP_RANGE_DESC_SCRIPT: &str = r#"
local key       = KEYS[1]
local max_score = ARGV[1]
local limit     = tonumber(ARGV[2])
return redis.call('ZREVRANGEBYSCORE', key, max_score, '-inf', 'WITHSCORES', 'LIMIT', 0, limit)
"#;

fn fred_err(e: fred::error::Error) -> TimelineError {
    TimelineError::Redis(redis_storage::RedisStorageError::from(e))
}

// ── RedisVipRegistry ──────────────────────────────────────────────────────────

pub struct RedisVipRegistry {
    client: RedisClient,
}

impl RedisVipRegistry {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl VipRegistry for RedisVipRegistry {
    async fn register(
        &self,
        entry:    &FeedEntry,
        cap:      u16,
        ttl_secs: u64,
    ) -> Result<(), TimelineError> {
        let key    = vip_key(&entry.author_id);
        let score  = entry.published_at_ms.to_string();
        let member = entry.post_id.to_string();

        let _: i64 = self
            .client
            .inner
            .eval(
                VIP_ZADD_CAP_TTL_SCRIPT,
                vec![key],
                vec![score, member, cap.to_string(), ttl_secs.to_string()],
            )
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn deregister(
        &self,
        author_id: &AuthorId,
        post_id:   &PostId,
    ) -> Result<(), TimelineError> {
        let key    = vip_key(author_id);
        let member = post_id.to_string();

        const ZREM_SCRIPT: &str = r#"
return redis.call('ZREM', KEYS[1], ARGV[1])
"#;
        let _: i64 = self
            .client
            .inner
            .eval(ZREM_SCRIPT, vec![key], vec![member])
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn range_desc(
        &self,
        author_id:           &AuthorId,
        max_score_inclusive: i64,
        limit:               usize,
    ) -> Result<Vec<FeedEntry>, TimelineError> {
        let key = vip_key(author_id);

        let raw: Vec<String> = self
            .client
            .inner
            .eval(
                VIP_RANGE_DESC_SCRIPT,
                vec![key],
                vec![max_score_inclusive.to_string(), limit.to_string()],
            )
            .await
            .map_err(fred_err)?;

        if raw.len() % 2 != 0 {
            return Err(TimelineError::ScriptReturnInvalid { context: "vip_range_desc interleaved parse" });
        }

        let author_id_copy = *author_id;
        raw.chunks_exact(2)
            .map(|chunk| {
                let post_id = PostId::try_from(chunk[0].as_str())?;
                let score   = chunk[1].parse::<f64>().map_err(|_| {
                    TimelineError::ScriptReturnInvalid { context: "vip_range_desc score parse" }
                })?;
                Ok(FeedEntry::new(post_id, author_id_copy, score as i64))
            })
            .collect()
    }

    async fn exists(&self, author_id: &AuthorId) -> Result<bool, TimelineError> {
        let exists: bool = self
            .client
            .inner
            .exists(vip_key(author_id))
            .await
            .map_err(fred_err)?;
        Ok(exists)
    }
}
