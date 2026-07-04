//! Scenario — following-set cache invalidation.
//!
//! On a cold following-set the read path rebuilds it from social-graph over gRPC
//! and persists it to Redis; a subsequent read is served entirely from the cache
//! and must NOT call social-graph again. This is the cache-invalidation axis: the
//! cache population is the invalidation boundary, asserted via the fake's call
//! counter.

use crate::timeline_it::harness::{self, HarnessOptions, TestHarness, DEADLINE};

#[tokio::test]
async fn following_set_is_rebuilt_once_then_served_from_cache() {
    let h = TestHarness::start(HarnessOptions::default()).await;

    let reader = harness::random_profile();
    let author = harness::random_author();
    h.social_graph.add_follow(reader, author);

    // First read: cold following-set → exactly one social-graph rebuild.
    let _ = h.get_following_feed(&reader).await;
    assert_eq!(
        h.social_graph.following_calls(),
        1,
        "cold following-set must trigger exactly one social-graph rebuild",
    );

    // The set is now cached in Redis.
    let following_store = h.following_store.clone();
    harness::await_until("following-set persisted to Redis", DEADLINE, || {
        let following_store = following_store.clone();
        async move { following_store.exists(&reader).await.unwrap_or(false) }
    })
    .await;

    // Second read: served from cache — no further social-graph traffic.
    let _ = h.get_following_feed(&reader).await;
    assert_eq!(
        h.social_graph.following_calls(),
        1,
        "a warm following-set must be served from Redis without re-calling social-graph",
    );
}
