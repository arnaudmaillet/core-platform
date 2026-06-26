//! Trending ranking over the real sorted-set board (`ZINCRBY` / `ZREVRANGE`).

use counter::domain::{EntityRef, Metric};

use super::super::harness::{Harness, fresh_post, view};

#[tokio::test]
async fn ranks_entities_by_score() {
    let h = Harness::start().await;
    let low = fresh_post();
    let mid = fresh_post();
    let high = fresh_post();

    h.ingest((0..10).map(|i| view(&low, 1_000 + i)).collect()).await;
    h.ingest((0..100).map(|i| view(&high, 1_000 + i)).collect()).await;
    h.ingest((0..50).map(|i| view(&mid, 1_000 + i)).collect()).await;

    // The global board is shared across parallel scenarios, so assert the relative
    // order of *our* three entities rather than absolute positions.
    let board = h.top_k(Metric::View, 10_000).await;
    let ids: Vec<&str> = board.iter().map(|t| t.entity.id.as_str()).collect();
    let pos = |e: &EntityRef| {
        ids.iter()
            .position(|id| *id == e.id.as_str())
            .unwrap_or_else(|| panic!("entity {} missing from board", e.id.as_str()))
    };

    assert!(pos(&high) < pos(&mid), "high should outrank mid");
    assert!(pos(&mid) < pos(&low), "mid should outrank low");
}
