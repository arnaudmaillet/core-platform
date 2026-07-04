//! Scenario — gRPC broadcast stream lifetime (RAII reclamation + refcount).
//!
//! A streaming subscriber shares one `broadcast::Sender` per profile. Dropping
//! the last receiver must leave the sender reclaimable by the registry reaper,
//! and the sender must survive while *any* subscriber remains — the same
//! refcount discipline chat enforces on its planes. This is the stream-lifetime
//! axis: connect, deliver, disconnect, reclaim.

use crate::notification_it::harness::{self, TestHarness, DEADLINE};

/// A delivered notification reaches the stream, and dropping the stream makes the
/// registry sender reclaimable.
#[tokio::test]
async fn stream_delivers_then_drop_reclaims_sender() {
    let h = TestHarness::start().await;

    let target = harness::random_profile();
    let sender = harness::random_profile();

    let mut stream = h.open_stream(&target).await;

    // A create for the target must fan out to the live stream.
    h.create(&target, &sender).await;

    let mut delivered = false;
    while let Some(item) = harness::recv(&mut stream, DEADLINE).await {
        let resp = item.expect("stream yielded a non-ok status");
        if resp.notification.is_some() {
            delivered = true;
            break;
        }
    }
    assert!(delivered, "live stream never delivered the created notification");

    // Abrupt disconnect — the receiver drops with the stream.
    drop(stream);

    // The zero-receiver sender is now reclaimable by the reaper.
    let registry = h.stream_registry.clone();
    harness::await_until("registry sender reclaimable after disconnect", DEADLINE, || {
        let registry = registry.clone();
        async move { registry.reap() >= 1 }
    })
    .await;
}

/// The per-profile sender is refcounted: it survives one of two subscribers
/// leaving and only becomes reclaimable when the last one does.
#[tokio::test]
async fn sender_refcount_survives_until_last_subscriber_leaves() {
    let h = TestHarness::start().await;

    let target = harness::random_profile();
    let sender = harness::random_profile();

    let s1 = h.open_stream(&target).await;
    let mut s2 = h.open_stream(&target).await;

    // Drop one of two subscribers. The sender must stay live — proven by a fresh
    // notification still reaching the surviving stream.
    drop(s1);
    h.create(&target, &sender).await;

    let mut delivered = false;
    while let Some(item) = harness::recv(&mut s2, DEADLINE).await {
        let resp = item.expect("stream yielded a non-ok status");
        if resp.notification.is_some() {
            delivered = true;
            break;
        }
    }
    assert!(
        delivered,
        "sender was wrongly torn down after only one of two subscribers left",
    );

    // Drop the last subscriber → the sender must now be reclaimable.
    drop(s2);
    let registry = h.stream_registry.clone();
    harness::await_until("sender reclaimable after last subscriber left", DEADLINE, || {
        let registry = registry.clone();
        async move { registry.reap() >= 1 }
    })
    .await;
}
