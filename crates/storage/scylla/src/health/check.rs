use scylla::client::caching_session::CachingSession;
use tracing::instrument;

use crate::error::ScyllaStorageError;

/// Performs a lightweight liveness probe against the ScyllaDB cluster.
///
/// Issues a single `SELECT` against `system.local` — a built-in system table
/// that is always present on every ScyllaDB node — using the session's default
/// execution profile. The query is intentionally minimal: one row, one column,
/// no schema changes.
///
/// ## When to call
///
/// - gRPC health check handlers (`grpc.health.v1.Health/Check`)
/// - Kubernetes liveness and readiness probes
/// - Startup diagnostics before accepting traffic
///
/// ## Errors
///
/// Any connectivity or consistency error surfaces as a
/// [`ScyllaStorageError`] and should cause the health check to report
/// `NOT_SERVING`.
#[instrument(name = "scylla.health_check", skip_all, err)]
pub async fn health_check(session: &CachingSession) -> Result<(), ScyllaStorageError> {
    session
        .get_session()
        .query_unpaged("SELECT key FROM system.local LIMIT 1", ())
        .await
        .map_err(ScyllaStorageError::from)?;
    Ok(())
}
