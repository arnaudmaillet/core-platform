//! Scenario — VIP routing (the author-tier fix, P4).
//!
//! The whole point of the author-tier initiative: a VIP author's post must NOT be
//! fanned out on write to every follower's feed (the celebrity meltdown), but be
//! served at read time from the per-author VIP registry. This proves the last mile
//! — given a `post.v1.events` event stamped with the VIP tier (which the
//! social-graph→profile→post producer chain now supplies), timeline routes to its
//! read path and writes nothing to follower feeds.

use crate::timeline_it::harness::{self, HarnessOptions, TestHarness};

#[tokio::test]
async fn vip_post_uses_read_path_not_write_fanout() {
    let h = TestHarness::start(HarnessOptions::default()).await;

    let reader = harness::random_profile();
    let vip_author = harness::random_author();
    h.social_graph.add_follow(reader, vip_author);

    // Ingest a VIP-tier post (the tier post now stamps onto post.v1.events).
    h.ingest_post(&vip_author, harness::TIER_VIP, 1_000).await;

    // Write path NOT taken: the follower's materialized feed stays empty — a
    // celebrity is never fanned out to millions of feeds.
    let materialized = h
        .feed_store
        .range_desc(&reader, i64::MAX, 10)
        .await
        .expect("range_desc");
    assert!(
        materialized.is_empty(),
        "a VIP author's post must not be fanned out to follower feeds"
    );

    // Read path DOES surface it: get_following_feed merges the VIP registry.
    let page = h.get_following_feed(&reader).await;
    assert!(
        page.items.iter().any(|e| e.published_at_ms == 1_000),
        "the VIP post must surface via the read-time VIP merge"
    );
}

#[tokio::test]
async fn standard_post_still_writes_to_follower_feeds() {
    let h = TestHarness::start(HarnessOptions::default()).await;

    let reader = harness::random_profile();
    let author = harness::random_author();
    h.social_graph.add_follow(reader, author);

    // A Standard author keeps the fan-out-on-write path (the contrast to VIP).
    h.ingest_post(&author, harness::TIER_STANDARD, 2_000).await;

    let materialized = h
        .feed_store
        .range_desc(&reader, i64::MAX, 10)
        .await
        .expect("range_desc");
    assert!(
        materialized.iter().any(|e| e.published_at_ms == 2_000),
        "a Standard author's post must be fanned out to follower feeds"
    );
}
