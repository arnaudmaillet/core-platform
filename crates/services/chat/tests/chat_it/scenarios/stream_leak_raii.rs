//! Scenario 2 — RAII Stream-Leak Protection.
//!
//! Dropping a stream (a client disconnect) must release its resources via the
//! stream guard's `Drop`: Member-Plane presence is cleared and the in-process
//! sender becomes reclaimable; on the Audience Plane the refcounted Redis
//! subscription only tears down (and deactivates the shard) when the *last* local
//! subscriber leaves.
//!
//! Driving the handler in-process lets us drop a stream at a precise instant —
//! the abrupt-disconnect simulation — and then poll durable state for the
//! asynchronously-spawned cleanup.

use crate::chat_it::harness::{self, HarnessOptions, TestHarness, DEADLINE};

/// Member-Plane: a dropped stream clears presence and frees the in-process sender.
#[tokio::test]
async fn member_stream_drop_clears_presence_and_reclaims_sender() {
    // Long TTLs so nothing ages out on its own — only the explicit `leave` clears
    // presence, making the assertion about `Drop`, not expiry.
    let opts = HarnessOptions { presence_ttl_secs: 300, audience_ttl_secs: 300, ..Default::default() };
    let h = TestHarness::start(opts).await;

    let owner = harness::random_profile();
    let conv = h.create_public_channel(&owner).await;

    let stream = h.open_member_stream(&conv, &owner).await;

    let presence = h.presence.clone();
    harness::await_until("presence registered on connect", DEADLINE, || {
        let presence = presence.clone();
        async move {
            presence
                .online(&conv, harness::now_ms(), 300)
                .await
                .map(|members| members.contains(&owner))
                .unwrap_or(false)
        }
    })
    .await;

    // Abrupt disconnect.
    drop(stream);

    harness::await_until("presence cleared on disconnect", DEADLINE, || {
        let presence = presence.clone();
        async move {
            presence
                .online(&conv, harness::now_ms(), 300)
                .await
                .map(|members| !members.contains(&owner))
                .unwrap_or(false)
        }
    })
    .await;

    // The in-process broadcast sender now has zero receivers and is reclaimable.
    assert!(
        h.member_registry.reap() >= 1,
        "member-plane sender was not reclaimable after the stream dropped",
    );
}

/// Audience-Plane: shard activation is refcounted — it survives one of two
/// subscribers leaving and only deactivates (Redis `SUNSUBSCRIBE` →
/// `deactivate_shard`) when the last one does.
#[tokio::test]
async fn audience_shard_refcount_survives_until_last_subscriber_leaves() {
    // shard_count = 1 (default) → both guests land on shard 0. Long TTL so the
    // shard cannot re-appear via a heartbeat and mask a refcount bug.
    let opts = HarnessOptions { audience_ttl_secs: 300, ..Default::default() };
    let h = TestHarness::start(opts).await;

    let owner = harness::random_profile();
    let conv = h.create_public_channel(&owner).await;

    let g1 = harness::random_profile();
    let g2 = harness::random_profile();

    let s1 = h.open_public_stream(&conv, &g1).await;
    let routing = h.routing.clone();
    harness::await_until("shard 0 activated by first subscriber", DEADLINE, || {
        let routing = routing.clone();
        async move {
            routing
                .active_shards(&conv, harness::now_ms(), 300)
                .await
                .map(|shards| shards.contains(&0u16))
                .unwrap_or(false)
        }
    })
    .await;

    let mut s2 = h.open_public_stream(&conv, &g2).await;

    // Drop one of two. The shard must stay active — proven by a fresh message
    // still fanning out to the surviving subscriber.
    drop(s1);
    h.send_text(&conv, &owner, "after-s1").await;

    let mut delivered = false;
    while let Some(item) = harness::recv(&mut s2, DEADLINE).await {
        let resp = item.expect("StreamPublic yielded a non-ok status");
        if resp.message.is_some_and(|m| m.body == "after-s1") {
            delivered = true;
            break;
        }
    }
    assert!(
        delivered,
        "shard was wrongly deactivated after only one of two subscribers left",
    );

    // Drop the last subscriber → the shard must now deactivate.
    drop(s2);
    harness::await_until("shard deactivated after last subscriber left", DEADLINE, || {
        let routing = routing.clone();
        async move {
            routing
                .active_shards(&conv, harness::now_ms(), 300)
                .await
                .map(|shards| shards.is_empty())
                .unwrap_or(false)
        }
    })
    .await;

    assert!(
        h.audience_registry.reap() >= 1,
        "audience-plane sender was not reclaimable after all subscribers left",
    );
}
