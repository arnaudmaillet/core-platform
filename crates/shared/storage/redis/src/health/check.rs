use fred::interfaces::{ClientLike, HeartbeatInterface};
use tracing::instrument;

use crate::error::map::RedisStorageError;

/// Performs a lightweight liveness probe against the Redis server.
///
/// Issues a `PING` command — the cheapest possible round-trip — and asserts
/// that the server returns `PONG`. The command goes through the normal command
/// pipeline, so it also validates that fred's internal multiplexer and the
/// connection's write/read path are healthy.
///
/// ## When to call
///
/// - gRPC health check handlers (`grpc.health.v1.Health/Check`)
/// - Kubernetes liveness and readiness probes
/// - Startup diagnostics before accepting traffic
///
/// ## Client compatibility
///
/// Accepts any type implementing [`ClientLike`] and [`HeartbeatInterface`] —
/// pass a [`RedisClient`] or a [`RedisPool`] interchangeably:
///
/// ```rust,ignore
/// use redis_storage::{RedisPoolBuilder, RedisConfig};
/// use redis_storage::health::health_check;
///
/// let pool = RedisPoolBuilder::new(RedisConfig::from_env()).build().await?;
/// health_check(&pool).await?;
/// ```
///
/// ## Errors
///
/// Any connectivity, timeout, or authentication error surfaces as a
/// [`RedisStorageError`] and should cause the health check to report
/// `NOT_SERVING`.
///
/// [`RedisClient`]: crate::client::builder::RedisClient
/// [`RedisPool`]:   crate::pool::builder::RedisPool
#[instrument(name = "redis.health_check", skip_all, err)]
pub async fn health_check<C>(client: &C) -> Result<(), RedisStorageError>
where
    C: ClientLike + HeartbeatInterface,
{
    client
        .ping::<()>(None)
        .await
        .map_err(RedisStorageError::from)
}
