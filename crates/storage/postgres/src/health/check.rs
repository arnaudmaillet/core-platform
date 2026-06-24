use crate::error::StorageError;
use crate::routing::{ShardCluster, ShardId};
use sqlx::PgPool;
use std::collections::HashMap;

/// Validates database reachability by issuing a no-op `SELECT 1`.
///
/// Intended for Kubernetes readiness probes and load-balancer health checks.
/// The round-trip includes connection acquisition from the pool, so it also
/// exercises pool liveness under the configured `acquire_timeout`.
///
/// Mount this behind `GET /healthz/ready` or equivalent in the service binary.
#[tracing::instrument(name = "postgres.health_check", skip(pool))]
pub async fn health_check(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(StorageError::from)
}

/// Validates reachability of every shard in a cluster concurrently.
///
/// All shards are probed in parallel via `SELECT 1` regardless of individual
/// failures — the returned map is always fully populated (one entry per shard).
///
/// A shard returning `Err` signals partial degradation; the health endpoint
/// is responsible for deciding whether to report liveness or readiness failure
/// based on how many shards are affected.
///
/// # Pool cloning
///
/// [`PgPool`] is an `Arc<PgPoolInner>` internally, so each `pool.clone()`
/// inside this function is O(1) and never opens a new connection.
#[tracing::instrument(name = "postgres.health_check_cluster", skip(cluster), fields(
    shard_count = cluster.shard_count(),
))]
pub async fn health_check_cluster(
    cluster: &ShardCluster,
) -> HashMap<ShardId, Result<(), StorageError>> {
    let pool_pairs: Vec<(ShardId, PgPool)> = cluster
        .pools()
        .map(|(&shard_id, pool)| (shard_id, pool.clone()))
        .collect();

    let futures = pool_pairs.into_iter().map(|(shard_id, pool)| async move {
        let result = health_check(&pool).await;
        (shard_id, result)
    });

    futures::future::join_all(futures)
        .await
        .into_iter()
        .collect()
}
