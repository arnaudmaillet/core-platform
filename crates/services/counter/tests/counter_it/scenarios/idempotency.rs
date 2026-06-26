//! The idempotency guarantee, proven against the real Postgres flush CTE: a
//! redelivered window advances nothing, across every tier.

use counter::domain::Metric;

use super::super::harness::{Harness, fresh_post, view};

#[tokio::test]
async fn redelivered_window_does_not_double_count() {
    let h = Harness::start().await;
    let post = fresh_post();

    // One window of 100 views, folded but not yet flushed.
    let deltas = h.fold((0..100).map(|i| view(&post, 1_000 + i)).collect());

    // First flush applies everywhere.
    let first = h.flush(&deltas).await;
    assert_eq!(first.applied, 1);
    assert_eq!(h.total(&post, Metric::View).await, Some(100));
    assert_eq!(h.read(&post, &[Metric::View]).await.get(Metric::View), Some(100));

    // The exact same window (same window_id) redelivered: the CTE's
    // ON CONFLICT DO NOTHING gates the total, and the flusher gates the hot write.
    let second = h.flush(&deltas).await;
    assert_eq!(second.applied, 0);
    assert_eq!(second.already_applied, 1);

    // Neither the durable total nor the hot counter moved.
    assert_eq!(h.total(&post, Metric::View).await, Some(100));
    assert_eq!(h.read(&post, &[Metric::View]).await.get(Metric::View), Some(100));
}
