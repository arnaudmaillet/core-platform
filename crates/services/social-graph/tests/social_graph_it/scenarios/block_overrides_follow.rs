//! Scenario — block overrides follow (ordering invariant).
//!
//! Blocking is authoritative over following: when a target blocks a follower, the
//! existing follow edge must be severed across the adjacency tables, and any
//! subsequent re-follow attempt must be rejected by the block gate. This is the
//! ordering axis — the relative order of follow vs block must resolve to "blocked
//! wins", regardless of the follow that preceded it.

use crate::social_graph_it::harness::{self, TestHarness, DEADLINE};

#[tokio::test]
async fn block_severs_existing_follow_and_gates_refollow() {
    let h = TestHarness::start().await;

    let follower = harness::random_profile();
    let target = harness::random_profile();

    // The follower follows the target — the edge is visible in both projections.
    h.follow(&follower, &target).await;
    harness::await_until("follow edge established", DEADLINE, || {
        let h = &h;
        async move { harness::contains(&h.followers(&target).await, &follower) }
    })
    .await;

    // The target blocks the follower — this must sever the existing follow.
    h.block(&target, &follower).await;
    harness::await_until("block severed the follow edge", DEADLINE, || {
        let h = &h;
        async move { !harness::contains(&h.followers(&target).await, &follower) }
    })
    .await;
    assert!(
        !harness::contains(&h.following(&follower).await, &target),
        "block must sever the reciprocal following edge too",
    );

    // A re-follow attempt by the blocked profile must be rejected.
    let refollow = harness::dispatch_follow(
        std::sync::Arc::clone(&h.command_bus),
        follower.as_str(),
        target.as_str(),
    )
    .await;
    assert!(refollow.is_err(), "a blocked profile must not be able to re-follow the blocker");

    // …and the adjacency stays severed.
    assert!(
        !harness::contains(&h.followers(&target).await, &follower),
        "the block gate must keep the follow edge absent after a rejected re-follow",
    );
}
