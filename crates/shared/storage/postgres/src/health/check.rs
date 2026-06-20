use crate::error::StorageError;
use sqlx::PgPool;

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
