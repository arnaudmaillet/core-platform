use std::time::Duration;

/// Post retention duration.
///
/// Governs both the ScyllaDB `USING TTL` value and the Redis card key `EX`.
/// Default is 48 hours. Premium creators may receive extended retention
/// (e.g. 7 days) signalled via the Kafka `post.published` event.
///
/// The maximum TTL accepted is 30 days (2 592 000 s) — beyond that the
/// space-amplification tradeoff with TWCS breaks down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionTtl(Duration);

impl RetentionTtl {
    pub const DEFAULT_SECS: u64 = 172_800; // 48 h
    pub const MAX_SECS:     u64 = 2_592_000; // 30 days

    pub fn default_ttl() -> Self {
        Self(Duration::from_secs(Self::DEFAULT_SECS))
    }

    pub fn from_secs(secs: u64) -> Self {
        let clamped = secs.min(Self::MAX_SECS).max(1);
        Self(Duration::from_secs(clamped))
    }

    /// Seconds as i32 for ScyllaDB `USING TTL` bind parameter.
    pub fn as_scylla_ttl(&self) -> i32 {
        self.0.as_secs().min(i32::MAX as u64) as i32
    }

    /// Seconds as u64 for Redis `EX` argument.
    pub fn as_redis_ex(&self) -> u64 {
        self.0.as_secs()
    }

    pub fn as_duration(&self) -> Duration {
        self.0
    }
}
