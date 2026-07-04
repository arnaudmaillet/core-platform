use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use error::AppError;

use crate::application::port::TimeSeriesStore;
use crate::domain::{TimeSeriesBucket, TimeSeriesQuery};
use crate::error::CounterError;

/// The query-bus wrapper around a validated [`TimeSeriesQuery`].
#[derive(Debug, Clone)]
pub struct RunTimeSeries {
    pub query: TimeSeriesQuery,
}

impl Query for RunTimeSeries {
    type Response = Vec<TimeSeriesBucket>;
}

/// Serves `GetTimeSeries` from the cold tier. This is the one read allowed to
/// touch Scylla; it is not on the feed path. Fails **open** to an empty series on a
/// transient store outage.
pub struct TimeSeriesHandler {
    series: Arc<dyn TimeSeriesStore>,
}

impl TimeSeriesHandler {
    pub fn new(series: Arc<dyn TimeSeriesStore>) -> Self {
        Self { series }
    }
}

impl QueryHandler<RunTimeSeries> for TimeSeriesHandler {
    type Error = CounterError;

    async fn handle(
        &self,
        envelope: Envelope<RunTimeSeries>,
    ) -> Result<Vec<TimeSeriesBucket>, Self::Error> {
        match self.series.range(&envelope.payload.query).await {
            Ok(buckets) => Ok(buckets),
            Err(e) if e.is_retryable() => Ok(Vec::new()), // degrade to empty
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{EntityId, EntityKind, EntityRef, Metric, TimeGranularity};

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    #[tokio::test]
    async fn returns_buckets_in_range() {
        let fx = Fixture::new();
        let t0 = Utc.timestamp_opt(1_000, 0).unwrap();
        let t1 = Utc.timestamp_opt(2_000, 0).unwrap();
        fx.series.seed_bucket(&post("p1"), Metric::View, t0, 5);
        fx.series.seed_bucket(&post("p1"), Metric::View, t1, 8);

        let q = TimeSeriesQuery::new(
            post("p1"),
            Metric::View,
            TimeGranularity::Hour,
            Utc.timestamp_opt(0, 0).unwrap(),
            Utc.timestamp_opt(10_000, 0).unwrap(),
        )
        .unwrap();
        let buckets = fx
            .time_series_handler()
            .handle(Envelope::new(Uuid::now_v7(), RunTimeSeries { query: q }))
            .await
            .unwrap();

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].value, 5);
        assert_eq!(buckets[1].value, 8);
    }
}
