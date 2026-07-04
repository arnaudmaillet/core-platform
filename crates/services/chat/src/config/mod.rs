/// Runtime configuration for the chat service.
///
/// All fields are loaded from environment variables at startup with safe
/// production defaults. No hot-reload — changing a value requires a restart to
/// avoid mid-flight divergence between the gRPC streaming workers and the
/// Redis/ScyllaDB adapters.
///
/// The knobs are forward-declared here so later phases (persistence, routing,
/// streaming) wire against a single configuration surface. Hard *domain*
/// invariants (e.g. the 500-member group cap) live in the domain layer, not
/// here, because they must not be weakened by an operator.
#[derive(Debug, Clone)]
pub struct ChatConfig {
    /// Maximum number of messages returned per `GetHistory` page. Server-enforced
    /// to keep guest history pulls bounded and prevent full-partition scans.
    pub max_page_size: i32,

    /// Number of most-recent messages kept in the per-conversation Redis hot-tail
    /// cache. "Load last page" is served from here so the live ScyllaDB write
    /// partition is never read by passive guests.
    pub hot_tail_cache_size: u16,

    /// Size of the ScyllaDB message-log time bucket, in hours. Bounds partition
    /// size and migrates the write hotspot across token ranges over time, so cold
    /// history reads land on different partitions than the live write tail.
    pub message_bucket_hours: u32,

    /// `tokio::sync::broadcast` channel capacity per active Member-Plane stream.
    /// Receivers that fall behind by this many events receive `Lagged`.
    pub member_stream_buffer_size: usize,

    /// `tokio::sync::broadcast` channel capacity per active Audience-Plane stream.
    /// Sized larger than the member buffer to absorb fan-out bursts to passive
    /// guests before forcing a reconnect-and-repage.
    pub audience_stream_buffer_size: usize,

    /// Number of sharded Audience-Plane sub-channels (`chat:{aud:<id>:<k>}`) a
    /// public conversation fans the shadow message out to. Spreads millions of
    /// guests across the Redis Cluster instead of pinning them to the
    /// conversation's home slot.
    pub audience_shard_count: u16,

    /// TTL (seconds) for ephemeral Member-Plane presence keys. Refreshed by the
    /// client heartbeat; on expiry the member is considered offline.
    pub presence_ttl_secs: u64,

    /// TTL (seconds) for the typing-indicator flag. Short by design — typing is
    /// a transient, best-effort, Member-Plane-only signal.
    pub typing_ttl_secs: u64,
}

impl ChatConfig {
    pub fn from_env() -> Self {
        Self {
            max_page_size:              env_i32("CHAT_MAX_PAGE_SIZE", 50),
            hot_tail_cache_size:        env_u16("CHAT_HOT_TAIL_CACHE_SIZE", 200),
            message_bucket_hours:       env_u32("CHAT_MESSAGE_BUCKET_HOURS", 24),
            member_stream_buffer_size:  env_usize("CHAT_MEMBER_STREAM_BUFFER_SIZE", 256),
            audience_stream_buffer_size: env_usize("CHAT_AUDIENCE_STREAM_BUFFER_SIZE", 1024),
            audience_shard_count:       env_u16("CHAT_AUDIENCE_SHARD_COUNT", 16),
            presence_ttl_secs:          env_u64("CHAT_PRESENCE_TTL_SECS", 30),
            typing_ttl_secs:            env_u64("CHAT_TYPING_TTL_SECS", 6),
        }
    }
}

fn env_u16(var: &str, default: u16) -> u16 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_u32(var: &str, default: u32) -> u32 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_u64(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_i32(var: &str, default: i32) -> i32 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_usize(var: &str, default: usize) -> usize {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
