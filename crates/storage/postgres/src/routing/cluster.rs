use super::{hash::deterministic_shard_id, ShardId, ShardKey};
use crate::error::StorageError;
use sqlx::PgPool;
use std::collections::HashMap;

/// A registry mapping each [`ShardId`] to its owned [`PgPool`].
///
/// Constructed once at startup by [`PgClusterBuilder`] and shared across all
/// clones of [`TransactionManager`] through an `Arc<Topology>`. The registry
/// is **immutable after construction** — shard membership changes require a
/// rolling restart.
///
/// # Clone semantics
///
/// `ShardCluster` itself is not `Clone`. It is accessed exclusively through
/// the `Arc<Topology>` inside [`TransactionManager`], so O(1) clone is
/// maintained at the manager level without duplicating pool maps.
///
/// [`PgClusterBuilder`]: crate::pool::builder::PgClusterBuilder
/// [`TransactionManager`]: crate::transaction::manager::TransactionManager
#[derive(Debug)]
pub struct ShardCluster {
    shards: HashMap<ShardId, PgPool>,
    shard_count: u16,
}

impl ShardCluster {
    /// Constructs a cluster from a pre-built shard map.
    ///
    /// `shard_count` must equal `shards.len()` and must be non-zero.
    /// These invariants are enforced by [`PgClusterBuilder`]; do not call
    /// this constructor directly in application code.
    pub(crate) fn new(shards: HashMap<ShardId, PgPool>, shard_count: u16) -> Self {
        debug_assert_eq!(
            shards.len(),
            usize::from(shard_count),
            "shard registry length ({}) must equal shard_count ({})",
            shards.len(),
            shard_count,
        );
        debug_assert!(shard_count > 0, "shard_count must be non-zero");
        Self { shards, shard_count }
    }

    /// The number of application-level shards in this cluster.
    #[inline]
    pub fn shard_count(&self) -> u16 {
        self.shard_count
    }

    /// Deterministically routes `key` to its owning pool.
    ///
    /// Returns [`StorageError::ShardNotFound`] if the registry is missing an
    /// entry for the computed shard — a configuration invariant violation that
    /// should never occur after a well-formed [`PgClusterBuilder`] construction.
    #[inline]
    pub fn resolve<K: ShardKey + ?Sized>(&self, key: &K) -> Result<&PgPool, StorageError> {
        let shard_id = deterministic_shard_id(key, self.shard_count);
        self.shards
            .get(&shard_id)
            .ok_or(StorageError::ShardNotFound { shard_id })
    }

    /// Returns the computed [`ShardId`] for `key` without performing a pool
    /// lookup.
    ///
    /// Useful for emitting the owning shard in tracing spans and log lines
    /// without paying for the full `HashMap` lookup.
    #[inline]
    pub fn shard_id_for<K: ShardKey + ?Sized>(&self, key: &K) -> ShardId {
        deterministic_shard_id(key, self.shard_count)
    }

    /// Iterates over all `(ShardId, PgPool)` pairs in the cluster.
    ///
    /// Iteration order is not guaranteed to be sorted — callers that need
    /// deterministic ordering (e.g., migration runners) should collect and
    /// sort by [`ShardId`].
    ///
    /// Used by health checks and DDL migration runners that must visit every
    /// shard.
    pub fn pools(&self) -> impl Iterator<Item = (&ShardId, &PgPool)> + '_ {
        self.shards.iter()
    }
}
