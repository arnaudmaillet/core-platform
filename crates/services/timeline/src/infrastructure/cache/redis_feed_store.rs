use async_trait::async_trait;
use fred::interfaces::{KeysInterface, LuaInterface};
use redis_storage::RedisClient;

use crate::application::port::FeedStore;
use crate::domain::aggregate::FeedEntry;
use crate::domain::value_object::{AuthorId, PostId, ProfileId};
use crate::error::TimelineError;

// ── Key builder ───────────────────────────────────────────────────────────────

fn feed_key(profile_id: &ProfileId) -> String {
    format!("timeline:feed:{}", profile_id)
}

// ── Member encoding ───────────────────────────────────────────────────────────
//
// Each ZSET member is encoded as "{post_id}:{author_id}" so that the BFF can
// identify the author without a secondary lookup when reading the hot feed.
// Score = published_at_ms (f64).

fn encode_member(post_id: &PostId, author_id: &AuthorId) -> String {
    format!("{}:{}", post_id, author_id)
}

fn decode_member(member: &str, score: f64) -> Result<FeedEntry, TimelineError> {
    let (post_str, author_str) = member.split_once(':').ok_or_else(|| {
        TimelineError::DomainViolation {
            field:   "feed_member".to_owned(),
            message: format!("malformed member: '{member}'"),
        }
    })?;
    let post_id   = PostId::try_from(post_str)?;
    let author_id = AuthorId::try_from(author_str)?;
    Ok(FeedEntry::new(post_id, author_id, score as i64))
}

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Atomically adds one post to a user's feed ZSET and enforces the cap.
///
/// KEYS[1] = timeline:feed:{profile_id}
/// ARGV[1] = score  (published_at_ms as integer string)
/// ARGV[2] = member ("{post_id}:{author_id}")
/// ARGV[3] = cap    (integer string, e.g. "500")
///
/// Returns: ZCARD after the operation.
const ZADD_CAP_SCRIPT: &str = r#"
local key    = KEYS[1]
local score  = ARGV[1]
local member = ARGV[2]
local cap    = tonumber(ARGV[3])

redis.call('ZADD', key, score, member)

local card = redis.call('ZCARD', key)
if card > cap then
    redis.call('ZREMRANGEBYRANK', key, 0, card - cap - 1)
end

return redis.call('ZCARD', key)
"#;

/// Returns at most `limit` entries below `max_score` sorted newest-first.
///
/// KEYS[1] = timeline:feed:{profile_id}
/// ARGV[1] = max_score (integer string, inclusive upper bound)
/// ARGV[2] = limit     (integer string)
///
/// Returns: interleaved [member1, score1, member2, score2, ...] list.
/// The Lua ZREVRANGEBYSCORE WITHSCORES returns member/score pairs in order.
const RANGE_DESC_SCRIPT: &str = r#"
local key       = KEYS[1]
local max_score = ARGV[1]
local limit     = tonumber(ARGV[2])
return redis.call('ZREVRANGEBYSCORE', key, max_score, '-inf', 'WITHSCORES', 'LIMIT', 0, limit)
"#;

fn fred_err(e: fred::error::Error) -> TimelineError {
    TimelineError::Redis(redis_storage::RedisStorageError::from(e))
}

// ── RedisFeedStore ────────────────────────────────────────────────────────────

pub struct RedisFeedStore {
    client: RedisClient,
}

impl RedisFeedStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl FeedStore for RedisFeedStore {
    async fn push(
        &self,
        profile_id: &ProfileId,
        entry:      &FeedEntry,
        cap:        u16,
    ) -> Result<(), TimelineError> {
        let key    = feed_key(profile_id);
        let score  = entry.published_at_ms.to_string();
        let member = encode_member(&entry.post_id, &entry.author_id);

        let _: i64 = self
            .client
            .inner
            .eval(
                ZADD_CAP_SCRIPT,
                vec![key],
                vec![score, member, cap.to_string()],
            )
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn push_batch(
        &self,
        profile_id: &ProfileId,
        entries:    &[FeedEntry],
        cap:        u16,
    ) -> Result<(), TimelineError> {
        for entry in entries {
            self.push(profile_id, entry, cap).await?;
        }
        Ok(())
    }

    async fn remove_post(
        &self,
        profile_id: &ProfileId,
        post_id:    &PostId,
    ) -> Result<(), TimelineError> {
        // Without the author_id we cannot reconstruct the exact member string.
        // Use ZRANGEBYSCORE + filter in Lua to find and remove the post_id prefix.
        // For single-item removal, scan all members matching the post_id prefix.
        let key = feed_key(profile_id);
        let prefix = format!("{}:", post_id);

        // Lua scan: iterate all members, remove those starting with the post_id.
        const SCAN_REMOVE_SCRIPT: &str = r#"
local key    = KEYS[1]
local prefix = ARGV[1]
local all    = redis.call('ZRANGE', key, 0, -1)
local removed = 0
for _, m in ipairs(all) do
    if m:sub(1, #prefix) == prefix then
        removed = removed + redis.call('ZREM', key, m)
    end
end
return removed
"#;
        let _: i64 = self
            .client
            .inner
            .eval(SCAN_REMOVE_SCRIPT, vec![key], vec![prefix])
            .await
            .map_err(fred_err)?;
        Ok(())
    }

    async fn remove_posts_batch(
        &self,
        profile_id: &ProfileId,
        post_ids:   &[PostId],
    ) -> Result<(), TimelineError> {
        if post_ids.is_empty() {
            return Ok(());
        }
        // Build prefix list and remove matching members via Lua.
        const BATCH_PREFIX_REMOVE: &str = r#"
local key      = KEYS[1]
local n_prefix = tonumber(ARGV[1])
local prefixes = {}
for i = 2, n_prefix + 1 do
    prefixes[i - 1] = ARGV[i]
end
local all     = redis.call('ZRANGE', key, 0, -1)
local removed = 0
for _, m in ipairs(all) do
    for _, prefix in ipairs(prefixes) do
        if m:sub(1, #prefix) == prefix then
            removed = removed + redis.call('ZREM', key, m)
            break
        end
    end
end
return removed
"#;
        let key   = feed_key(profile_id);
        let count = post_ids.len().to_string();
        let mut args: Vec<String> = vec![count];
        for pid in post_ids {
            args.push(format!("{}:", pid));
        }

        let _: i64 = self
            .client
            .inner
            .eval(BATCH_PREFIX_REMOVE, vec![key], args)
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn range_desc(
        &self,
        profile_id:          &ProfileId,
        max_score_inclusive: i64,
        limit:               usize,
    ) -> Result<Vec<FeedEntry>, TimelineError> {
        let key = feed_key(profile_id);

        // Returns interleaved [member, score, member, score, ...].
        let raw: Vec<String> = self
            .client
            .inner
            .eval(
                RANGE_DESC_SCRIPT,
                vec![key],
                vec![max_score_inclusive.to_string(), limit.to_string()],
            )
            .await
            .map_err(fred_err)?;

        parse_interleaved(raw)
    }

    async fn exists(&self, profile_id: &ProfileId) -> Result<bool, TimelineError> {
        let exists: bool = self
            .client
            .inner
            .exists(feed_key(profile_id))
            .await
            .map_err(fred_err)?;
        Ok(exists)
    }
}

/// Parses interleaved [member, score, member, score, ...] Lua output.
fn parse_interleaved(raw: Vec<String>) -> Result<Vec<FeedEntry>, TimelineError> {
    if raw.len() % 2 != 0 {
        return Err(TimelineError::ScriptReturnInvalid { context: "range_desc interleaved parse" });
    }
    raw.chunks_exact(2)
        .map(|chunk| {
            let member = &chunk[0];
            let score  = chunk[1].parse::<f64>().map_err(|_| TimelineError::ScriptReturnInvalid {
                context: "range_desc score parse",
            })?;
            decode_member(member, score)
        })
        .collect()
}
