//! Scenario — cold→warm warm-up lifecycle under a concurrent read stampede.
//!
//! A cold feed is served from ScyllaDB (`is_cold = true`) while a bounded,
//! single-flighted background task warms the Redis feed and sets the warm flag.
//! This is the async-task-lifetime axis: many concurrent cold readers must all
//! get correct data, the warm-up must converge, and a subsequent read must be
//! served hot from Redis (`is_cold = false`).

use std::sync::Arc;

use crate::timeline_it::harness::{self, HarnessOptions, TestHarness, DEADLINE};

#[tokio::test]
async fn concurrent_cold_reads_converge_to_a_warm_feed() {
    let h = TestHarness::start(HarnessOptions::default()).await;

    let reader = harness::random_profile();
    let author = harness::random_author();
    h.social_graph.add_follow(reader, author);

    // One published post, fanned out to ScyllaDB (cold source) and Redis.
    h.ingest_post(&author, harness::TIER_STANDARD, 5_000).await;

    // Fire a stampede of cold reads concurrently; the warming guard must admit at
    // most one rebuild while every reader still gets correct data.
    let mut handles = Vec::new();
    for _ in 0..8 {
        let bus = Arc::clone(&h.query_bus);
        let profile_id = reader.as_uuid().to_string();
        handles.push(tokio::spawn(harness::dispatch_following(bus, profile_id)));
    }
    for handle in handles {
        let page = handle.await.expect("join").expect("following feed");
        assert!(
            !page.items.is_empty(),
            "a concurrent cold reader received an empty feed",
        );
    }

    // The single-flighted warm-up must converge: the warm flag gets set.
    let tier_cache = h.tier_cache.clone();
    harness::await_until("feed warmed (warm flag set)", DEADLINE, || {
        let tier_cache = tier_cache.clone();
        async move { tier_cache.is_warm(&reader).await.unwrap_or(false) }
    })
    .await;

    // A subsequent read is now served hot from Redis.
    let page = h.get_following_feed(&reader).await;
    assert!(!page.is_cold, "read after warm-up must be served hot from Redis");
    assert!(!page.items.is_empty(), "warm feed must still contain the post");
}
