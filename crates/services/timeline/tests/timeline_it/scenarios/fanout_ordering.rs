//! Scenario — fan-out-on-write: temporal ordering + cap eviction.
//!
//! Ingesting published posts for authors a profile follows materializes the
//! follower's Redis feed. The feed ZSET is ordered newest-first and bounded by
//! `feed_cap`: once the cap is exceeded the oldest entry is evicted. This is the
//! temporal-partitioning axis — score order must hold and the window must stay
//! bounded under a burst of writes.

use crate::timeline_it::harness::{self, HarnessOptions, TestHarness, DEADLINE};

#[tokio::test]
async fn fanout_materializes_feed_newest_first_and_enforces_cap() {
    // Cap of 2 so a third post must evict the oldest.
    let h = TestHarness::start(HarnessOptions { feed_cap: 2, ..Default::default() }).await;

    let reader = harness::random_profile();
    let a1 = harness::random_author();
    let a2 = harness::random_author();
    let a3 = harness::random_author();

    // The reader follows all three authors (drives fan-out targeting).
    h.social_graph.add_follow(reader, a1);
    h.social_graph.add_follow(reader, a2);
    h.social_graph.add_follow(reader, a3);

    // Publish oldest → newest.
    h.ingest_post(&a1, harness::TIER_STANDARD, 1_000).await;
    h.ingest_post(&a2, harness::TIER_STANDARD, 2_000).await;
    h.ingest_post(&a3, harness::TIER_STANDARD, 3_000).await;

    let feed_store = h.feed_store.clone();
    harness::await_until("feed materialized and capped at 2", DEADLINE, || {
        let feed_store = feed_store.clone();
        async move {
            feed_store
                .range_desc(&reader, i64::MAX, 10)
                .await
                .map(|entries| entries.len() == 2)
                .unwrap_or(false)
        }
    })
    .await;

    let entries = h
        .feed_store
        .range_desc(&reader, i64::MAX, 10)
        .await
        .expect("range_desc");

    // Newest-first, and the oldest (t=1_000) was evicted by the cap.
    let scores: Vec<i64> = entries.iter().map(|e| e.published_at_ms).collect();
    assert_eq!(scores, vec![3_000, 2_000], "feed must be newest-first with the oldest evicted");
}
