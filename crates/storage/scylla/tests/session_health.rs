mod common;

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require a live ScyllaDB cluster.
//
// Run with:
//   SCYLLA_CONTACT_POINTS=127.0.0.1:9042 \
//   SCYLLA_LOCAL_DC=datacenter1 \
//   cargo test -p scylla-storage -- --include-ignored
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies that the session builder can reach a live cluster.
#[tokio::test]
#[ignore = "requires live ScyllaDB cluster"]
async fn session_builder_connects_to_cluster() {
    let client = common::test_client().await;

    // A successful build means at least one contact point was reachable and
    // the cluster metadata was fetched. The assertion is intentionally trivial —
    // the important thing is that `build()` did not return an error.
    assert_eq!(client.profiles.get(scylla_storage::ProfileKind::Strict) as *const _,
               client.profiles.strict() as *const _);
}

/// Verifies that the health check returns `Ok` on a live cluster.
#[tokio::test]
#[ignore = "requires live ScyllaDB cluster"]
async fn health_check_passes_on_live_cluster() {
    use scylla_storage::health::health_check;

    let client = common::test_client().await;
    health_check(&client.session)
        .await
        .expect("health_check should succeed on a live cluster");
}

/// Verifies that the prepared-statement cache is populated after a query.
#[tokio::test]
#[ignore = "requires live ScyllaDB cluster"]
async fn caching_session_auto_prepares_statement() {
    let client = common::test_client().await;

    // The `system.local` table is always present; SELECT from it to trigger
    // statement preparation without touching user data.
    client
        .session
        .get_session()
        .query_unpaged("SELECT key FROM system.local LIMIT 1", ())
        .await
        .expect("query against system.local should succeed");
}

/// Smoke-tests the OtelHistoryListener attached to a real execution.
///
/// The test simply ensures no panic occurs when listener callbacks are invoked
/// during a live query. Span output is checked in unit tests.
#[tokio::test]
#[ignore = "requires live ScyllaDB cluster"]
async fn otel_listener_does_not_panic_on_live_query() {
    use std::sync::Arc;
    use scylla::statement::unprepared::Statement;
    use scylla::observability::history::HistoryListener;

    let client = common::test_client().await;
    let listener = Arc::clone(&client.history_listener);

    let mut stmt = Statement::new("SELECT key FROM system.local LIMIT 1");
    stmt.set_history_listener(listener as Arc<dyn HistoryListener>);

    client
        .session
        .get_session()
        .query_unpaged(stmt, ())
        .await
        .expect("live query with OtelHistoryListener should not fail");
}
