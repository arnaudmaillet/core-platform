//! Shared Lua scripts for the expiring-sorted-set pattern used by presence,
//! typing, and the audience-shard routing registry. All three are "a set of
//! members each kept alive by periodic heartbeats", so they share one
//! implementation and differ only in key, member, and TTL.

/// Heartbeats a member into an expiring sorted set, prunes anything older than
/// the liveness cutoff, and refreshes a key-level safety TTL.
///
/// KEYS[1] = the sorted-set key
/// ARGV[1] = now_ms        (score)
/// ARGV[2] = member
/// ARGV[3] = min_ms        (liveness cutoff = now_ms - ttl_ms; prune <= this)
/// ARGV[4] = key_ttl_secs  (safety expiry so an idle conversation's key is reclaimed)
pub(crate) const ZSET_HEARTBEAT: &str = r#"
local key = KEYS[1]
redis.call('ZADD', key, ARGV[1], ARGV[2])
redis.call('ZREMRANGEBYSCORE', key, 0, ARGV[3])
redis.call('EXPIRE', key, tonumber(ARGV[4]))
return 1
"#;

/// Returns the members still alive (score >= min_ms), newest-first.
///
/// KEYS[1] = the sorted-set key
/// ARGV[1] = min_ms (liveness cutoff = now_ms - ttl_ms)
pub(crate) const ZSET_ACTIVE: &str = r#"
return redis.call('ZREVRANGEBYSCORE', KEYS[1], '+inf', ARGV[1])
"#;

/// Removes a single member (clean leave / deactivate).
///
/// KEYS[1] = the sorted-set key
/// ARGV[1] = member
pub(crate) const ZSET_REMOVE: &str = r#"
redis.call('ZREM', KEYS[1], ARGV[1])
return 1
"#;
