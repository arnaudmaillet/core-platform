//! Scenario — atomic reaction toggle (Redis Lua idempotency under concurrency).
//!
//! A profile's reaction to a post is a set-membership, not a counter: upserting
//! the same reaction repeatedly — even concurrently — must leave the post's score
//! exactly as if it were applied once, and removing it must zero it. The
//! single-round-trip Lua script is what makes this hold without a read-modify-write
//! race. This is the concurrency axis on the hot path.

use std::sync::Arc;

use crate::engagement_it::harness::{self, TestHarness, DEADLINE};

const DUPLICATES: usize = 8;

#[tokio::test]
async fn duplicate_and_concurrent_upserts_are_idempotent_then_removable() {
    let h = TestHarness::start().await;

    let post = harness::random_post();
    let profile = harness::random_profile();

    // One reaction establishes the baseline heart score for a single profile.
    h.upsert(&post, &profile, harness::KIND_HEART).await;
    let baseline = harness::heart_score(&h.snapshot(&post).await);
    assert!(baseline > 0, "a heart reaction must register a positive score");

    // A stampede of identical upserts (same profile, same kind) must not inflate
    // the score beyond the single-reaction baseline.
    let mut handles = Vec::new();
    for _ in 0..DUPLICATES {
        let bus = Arc::clone(&h.command_bus);
        handles.push(tokio::spawn(harness::dispatch_upsert(
            bus,
            post.as_str(),
            profile.as_str(),
            harness::KIND_HEART,
        )));
    }
    for handle in handles {
        handle.await.expect("join").expect("upsert_reaction");
    }

    let after = harness::heart_score(&h.snapshot(&post).await);
    assert_eq!(after, baseline, "duplicate/concurrent upserts must be idempotent");

    // Removing the reaction zeroes the score.
    h.remove(&post, &profile).await;
    let h = &h;
    let post = &post;
    harness::await_until("reaction removal zeroes the score", DEADLINE, || async move {
        harness::heart_score(&h.snapshot(post).await) == 0
    })
    .await;
}
