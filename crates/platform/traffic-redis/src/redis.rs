//! The Redis-backed [`ClaimSource`] and the [`QuotaBackend`] it powers.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use fred::interfaces::LuaInterface;
use redis_storage::RedisClient;
use traffic::{Quota, QuotaBackend, QuotaError, TrafficDecision};

use crate::{claim::ClaimSource, lease::LeaseBook};

/// Atomic claim against a per-`(key, window)` counter. A single key keeps the script
/// cluster-slot-safe; the `{key}` hash-tag colocates a key's windows on one slot.
const CLAIM_SCRIPT: &str = r#"
local budget = tonumber(ARGV[1])
local want = tonumber(ARGV[2])
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
if current >= budget then return 0 end
local grant = budget - current
if grant > want then grant = want end
redis.call('INCRBY', KEYS[1], grant)
redis.call('PEXPIRE', KEYS[1], tonumber(ARGV[3]))
return grant
"#;

fn window_key(key: &str, window: u64) -> String {
    format!("traffic:lease:{{{key}}}:{window}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// [`ClaimSource`] backed by a single atomic Redis Lua script.
pub struct RedisClaimSource {
    client: RedisClient,
}

impl RedisClaimSource {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ClaimSource for RedisClaimSource {
    async fn claim(
        &self,
        key: &str,
        window: u64,
        budget: u64,
        want: u64,
        ttl_ms: u64,
    ) -> Result<u64, QuotaError> {
        let granted: i64 = self
            .client
            .inner
            .eval(
                CLAIM_SCRIPT,
                vec![window_key(key, window)],
                vec![budget.to_string(), want.to_string(), ttl_ms.to_string()],
            )
            .await
            .map_err(|e| QuotaError(e.to_string()))?;
        Ok(granted.max(0) as u64)
    }
}

/// The distributed [`QuotaBackend`]: a local lease cache amortizing atomic Redis claims.
pub struct RedisLeaseBackend {
    book: LeaseBook,
    claims: RedisClaimSource,
}

impl RedisLeaseBackend {
    pub fn new(client: RedisClient) -> Self {
        Self { book: LeaseBook::new(), claims: RedisClaimSource::new(client) }
    }

    /// Drop idle lease entries — call periodically (the binary's prune loop) to bound memory.
    pub fn prune(&self, lease_ms: u64) {
        self.book.prune(now_ms(), lease_ms);
    }

    /// Keys with a live local lease — for a cardinality gauge.
    pub fn tracked_keys(&self) -> usize {
        self.book.tracked_keys()
    }
}

#[async_trait]
impl QuotaBackend for RedisLeaseBackend {
    async fn check(&self, key: &str, quota: Quota) -> Result<TrafficDecision, QuotaError> {
        self.book.check(key, quota, &self.claims, now_ms()).await
    }
}
