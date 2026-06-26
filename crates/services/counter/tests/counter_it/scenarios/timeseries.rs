//! Cold-tier rollup + range read over the real Scylla counter table.

use counter::domain::{Metric, TimeGranularity, TimeSeriesQuery};

use super::super::harness::{Harness, at, fresh_post, view};

#[tokio::test]
async fn rolls_windows_into_time_buckets() {
    let h = Harness::start().await;
    let post = fresh_post();

    // Two distinct windows that floor into the same hour bucket: window 0 (sum 2)
    // and window 1 (sum 1). The Scylla counter must accumulate them to 3.
    h.ingest(vec![view(&post, 1_000), view(&post, 2_000)]).await;
    h.ingest(vec![view(&post, 6_000)]).await;

    let query = TimeSeriesQuery::new(
        post.clone(),
        Metric::View,
        TimeGranularity::Hour,
        at(-1_000_000),
        at(3_600_000),
    )
    .unwrap();
    let buckets = h.range(&query).await;

    let total: i64 = buckets.iter().map(|b| b.value).sum();
    assert_eq!(total, 3, "hour bucket should accumulate both windows");
}
