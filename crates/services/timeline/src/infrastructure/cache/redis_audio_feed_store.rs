use fred::interfaces::LuaInterface;
use redis_storage::RedisClient;

use crate::application::port::{AudioFeedMember, AudioFeedStore};
use crate::domain::value_object::{AudioId, AuthorId, PostId};
use crate::error::TimelineError;

fn audio_feed_key(audio_id: &AudioId) -> String {
    format!("audio:feed:{}", audio_id)
}

fn encode_member(post_id: &PostId, author_id: &AuthorId) -> String {
    format!("{}:{}", post_id, author_id)
}

fn decode_member(member: &str, score: f64) -> Result<AudioFeedMember, TimelineError> {
    let (post_str, author_str) = member.split_once(':').ok_or_else(|| {
        TimelineError::DomainViolation {
            field:   "audio_feed_member".to_owned(),
            message: format!("malformed audio feed member: '{member}'"),
        }
    })?;
    let post_id   = PostId::try_from(post_str)?;
    let author_id = AuthorId::try_from(author_str)?;
    Ok(AudioFeedMember { post_id, author_id, published_at_ms: score as i64 })
}

const AUDIO_ZADD_CAP_SCRIPT: &str = r#"
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

const AUDIO_RANGE_DESC_SCRIPT: &str = r#"
local key       = KEYS[1]
local max_score = ARGV[1]
local limit     = tonumber(ARGV[2])
return redis.call('ZREVRANGEBYSCORE', key, max_score, '-inf', 'WITHSCORES', 'LIMIT', 0, limit)
"#;

const AUDIO_ZREM_PREFIX_SCRIPT: &str = r#"
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

fn fred_err(e: fred::error::Error) -> TimelineError {
    TimelineError::Redis(redis_storage::RedisStorageError::from(e))
}

pub struct RedisAudioFeedStore {
    client: RedisClient,
}

impl RedisAudioFeedStore {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

impl AudioFeedStore for RedisAudioFeedStore {
    async fn push(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        author_id:       &AuthorId,
        published_at_ms: i64,
        cap:             u16,
    ) -> Result<(), TimelineError> {
        let key    = audio_feed_key(audio_id);
        let score  = published_at_ms.to_string();
        let member = encode_member(post_id, author_id);

        let _: i64 = self
            .client
            .inner
            .eval(
                AUDIO_ZADD_CAP_SCRIPT,
                vec![key],
                vec![score, member, cap.to_string()],
            )
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn remove(
        &self,
        audio_id:        &AudioId,
        post_id:         &PostId,
        _published_at_ms: i64,
    ) -> Result<(), TimelineError> {
        let key    = audio_feed_key(audio_id);
        let prefix = format!("{}:", post_id);

        let _: i64 = self
            .client
            .inner
            .eval(AUDIO_ZREM_PREFIX_SCRIPT, vec![key], vec![prefix])
            .await
            .map_err(fred_err)?;

        Ok(())
    }

    async fn range(
        &self,
        audio_id:  &AudioId,
        before_ms: Option<i64>,
        limit:     u16,
    ) -> Result<Vec<AudioFeedMember>, TimelineError> {
        let key       = audio_feed_key(audio_id);
        let max_score = before_ms.unwrap_or(i64::MAX).to_string();

        let raw: Vec<String> = self
            .client
            .inner
            .eval(
                AUDIO_RANGE_DESC_SCRIPT,
                vec![key],
                vec![max_score, limit.to_string()],
            )
            .await
            .map_err(fred_err)?;

        if raw.len() % 2 != 0 {
            return Err(TimelineError::ScriptReturnInvalid {
                context: "audio_range_desc interleaved parse",
            });
        }

        raw.chunks_exact(2)
            .map(|chunk| {
                let score = chunk[1].parse::<f64>().map_err(|_| {
                    TimelineError::ScriptReturnInvalid {
                        context: "audio_range_desc score parse",
                    }
                })?;
                decode_member(&chunk[0], score)
            })
            .collect()
    }
}
