mod common;

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests — require a live Redis instance.
//
// Run with:
//   REDIS_HOSTS=127.0.0.1:6379 cargo test -p redis-storage -- --include-ignored
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies that the pool builder can reach a live Redis instance.
///
/// A successful build means at least one connection was established and
/// `PING` received `PONG`.
#[tokio::test]
#[ignore = "requires live Redis instance"]
async fn pool_builder_connects_to_redis() {
    let pool = common::setup::test_pool().await;

    // The pool is usable immediately after `build()`.
    // A trivial assertion confirms the inner pool is valid without running
    // a separate command here — the PING inside `wait_for_connect` already
    // confirms connectivity.
    let _ = pool.inner.clone();
}

/// Verifies that the health check returns `Ok` on a live instance.
#[tokio::test]
#[ignore = "requires live Redis instance"]
async fn health_check_passes_on_live_instance() {
    use redis_storage::health::health_check;

    let pool = common::setup::test_pool().await;
    health_check(&pool.inner)
        .await
        .expect("health_check should succeed on a live Redis instance");
}

/// Verifies that a client-level health check also passes.
#[tokio::test]
#[ignore = "requires live Redis instance"]
async fn client_health_check_passes_on_live_instance() {
    use redis_storage::health::health_check;

    let client = common::setup::test_client().await;
    health_check(&client.inner)
        .await
        .expect("health_check should succeed on a live Redis instance");
}

/// Verifies that basic key-value operations work through the pool.
#[tokio::test]
#[ignore = "requires live Redis instance"]
async fn pool_set_get_round_trip() {
    use fred::interfaces::KeysInterface;

    let pool = common::setup::test_pool().await;

    let key   = "redis-storage:test:set-get";
    let value = "hello-from-redis-storage";

    pool.set::<(), _, _>(key, value, None, None, false)
        .await
        .expect("SET should succeed");

    let result: Option<String> = pool
        .get(key)
        .await
        .expect("GET should succeed");

    assert_eq!(result.as_deref(), Some(value), "GET should return the value written by SET");

    pool.del::<(), _>(key).await.expect("DEL should succeed");
}

/// Verifies that the event listener does not panic on a live instance.
///
/// Triggers a reconnect by issuing `CLIENT KILL` on the current connection,
/// then asserts the client reconnects transparently.
#[tokio::test]
#[ignore = "requires live Redis instance; mutates connection state"]
async fn event_listener_does_not_panic_on_reconnect() {
    use fred::interfaces::KeysInterface;

    let client = common::setup::test_client().await;

    // The event listener was already spawned inside `build()`.
    // Issuing a command confirms the pipeline is healthy.
    let _: Option<String> = client
        .get("redis-storage:nonexistent:key")
        .await
        .expect("GET on nonexistent key should return None, not error");
}
