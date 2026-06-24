use std::fmt;

/// Opaque, zero-based shard index within an [`ApplicationSharded`] cluster.
///
/// The inner `u16` is the shard index in `[0, shard_count)`. Keeping it `u16`
/// bounds the maximum cluster size to 65 535 application-level shards — far
/// beyond any realistic deployment, while staying narrower than `usize` so
/// shard IDs never silently widen during index arithmetic.
///
/// [`ApplicationSharded`]: crate::config::TopologyConfig::ApplicationSharded
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShardId(pub u16);

impl ShardId {
    /// Returns the raw shard index.
    #[inline]
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ShardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shard-{}", self.0)
    }
}
