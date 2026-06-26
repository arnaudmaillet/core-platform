//! The firehose collapse, end-to-end over the real hot tier.

use counter::domain::Metric;

use super::super::harness::{Harness, fresh_post, unique_view, view};

#[tokio::test]
async fn collapses_views_into_live_total() {
    let h = Harness::start().await;
    let post = fresh_post();

    // 1000 views, all inside one window → one delta → one HINCRBY of +1000.
    let observations = (0..1000).map(|i| view(&post, 1_000 + i)).collect();
    let report = h.ingest(observations).await;
    assert_eq!(report.applied, 1);

    // Real Redis HINCRBY result.
    let snap = h.read(&post, &[Metric::View]).await;
    assert_eq!(snap.get(Metric::View), Some(1000));

    // And the durable total landed too.
    assert_eq!(h.total(&post, Metric::View).await, Some(1000));
}

#[tokio::test]
async fn estimates_unique_viewers_within_hll_error() {
    let h = Harness::start().await;
    let post = fresh_post();

    // 500 distinct viewers, each seen 3× — the duplicates must collapse.
    let mut observations = Vec::new();
    for v in 0..500 {
        for _ in 0..3 {
            observations.push(unique_view(&post, &format!("viewer-{v}"), 1_000));
        }
    }
    h.ingest(observations).await;

    // Real PFCOUNT over the HyperLogLog; ~0.81% std error, generous bound here.
    let snap = h.read(&post, &[Metric::UniqueViewer]).await;
    let estimate = snap.get(Metric::UniqueViewer).expect("unique viewer estimate");
    assert!(
        (estimate - 500).abs() <= 25,
        "HLL estimate {estimate} not within ±25 of 500"
    );
}
