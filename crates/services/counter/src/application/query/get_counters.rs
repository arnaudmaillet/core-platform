use std::sync::Arc;
use std::time::Duration;

use cqrs::{Envelope, Query, QueryHandler};
use error::AppError;

use crate::application::port::{CounterLedger, CounterStore};
use crate::domain::{BatchGetQuery, BatchReadout, CountSnapshot, CounterValue, EntityRef, Metric};
use crate::error::CounterError;

/// The query-bus wrapper around a validated [`BatchGetQuery`]. Validation / proto
/// mapping happens at the edge (Phase 5); by here the query is well-formed.
#[derive(Debug, Clone)]
pub struct RunBatchGet {
    pub query: BatchGetQuery,
}

impl Query for RunBatchGet {
    type Response = BatchReadout;
}

/// Serves `BatchGetCounters`. Reads the hot tier under a hard timeout; on a hot-tier
/// outage **or** a timeout it **fails open** to the warm ledger (stale-but-served)
/// rather than erroring the feed.
pub struct BatchGetHandler {
    hot: Arc<dyn CounterStore>,
    ledger: Arc<dyn CounterLedger>,
    read_timeout: Duration,
}

impl BatchGetHandler {
    pub fn new(
        hot: Arc<dyn CounterStore>,
        ledger: Arc<dyn CounterLedger>,
        read_timeout: Duration,
    ) -> Self {
        Self {
            hot,
            ledger,
            read_timeout,
        }
    }

    /// Rebuild snapshots from the durable ledger when the hot tier is unavailable.
    async fn fallback(
        &self,
        entities: &[EntityRef],
        metrics: &[Metric],
    ) -> Result<Vec<CountSnapshot>, CounterError> {
        let mut out = Vec::with_capacity(entities.len());
        for entity in entities {
            let mut values = Vec::new();
            for &metric in metrics {
                if let Some(total) = self.ledger.read_total(entity, metric).await? {
                    values.push(CounterValue::new(metric, total));
                }
            }
            out.push(CountSnapshot::new(entity.clone(), values));
        }
        Ok(out)
    }
}

impl QueryHandler<RunBatchGet> for BatchGetHandler {
    type Error = CounterError;

    async fn handle(&self, envelope: Envelope<RunBatchGet>) -> Result<BatchReadout, Self::Error> {
        let query = envelope.payload.query;
        let metrics = query.effective_metrics();

        // A slow hot tier must shed load rather than queue: bound the read and treat
        // an elapse exactly like a transient store fault — fail open to the ledger.
        let read = tokio::time::timeout(self.read_timeout, self.hot.read(&query.entities, &metrics));
        let degraded_fallback = || async {
            let snapshots = self.fallback(&query.entities, &metrics).await?;
            Ok(BatchReadout {
                snapshots,
                degraded: true,
            })
        };

        match read.await {
            Ok(Ok(snapshots)) => Ok(BatchReadout {
                snapshots,
                degraded: false,
            }),
            // Hot tier down → serve stale from the ledger. Only a transient store
            // fault degrades; a genuine fault (bad request, bug) still propagates.
            Ok(Err(e)) if e.is_retryable() => degraded_fallback().await,
            Ok(Err(e)) => Err(e),
            // Timed out → degrade.
            Err(_elapsed) => degraded_fallback().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{EntityId, EntityKind};

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    fn run(query: BatchGetQuery) -> Envelope<RunBatchGet> {
        Envelope::new(Uuid::now_v7(), RunBatchGet { query })
    }

    #[tokio::test]
    async fn reads_hot_tier() {
        let fx = Fixture::new();
        fx.hot.seed(&post("p1"), Metric::View, 42);
        fx.hot.seed(&post("p1"), Metric::Like, 7);

        let q = BatchGetQuery::new(vec![post("p1")], vec![Metric::View, Metric::Like]).unwrap();
        let out = fx.batch_get_handler().handle(run(q)).await.unwrap();

        assert!(!out.degraded);
        assert_eq!(out.snapshots.len(), 1);
        assert_eq!(out.snapshots[0].get(Metric::View), Some(42));
        assert_eq!(out.snapshots[0].get(Metric::Like), Some(7));
    }

    #[tokio::test]
    async fn fails_open_to_ledger_when_hot_unavailable() {
        let fx = Fixture::new();
        // Durable total exists; hot tier is down.
        fx.ledger.seed_total(&post("p1"), Metric::View, 1000);
        fx.hot.set_unavailable(true);

        let q = BatchGetQuery::new(vec![post("p1")], vec![Metric::View]).unwrap();
        let out = fx.batch_get_handler().handle(run(q)).await.unwrap();

        assert!(out.degraded); // stale-but-served
        assert_eq!(out.snapshots[0].get(Metric::View), Some(1000));
    }

    #[tokio::test]
    async fn slow_hot_tier_times_out_and_degrades() {
        let fx = Fixture::new();
        fx.ledger.seed_total(&post("p1"), Metric::View, 777);
        // Hot read is healthy but slow; the handler's hard timeout sheds it.
        fx.hot.set_read_delay(std::time::Duration::from_millis(200));

        let handler = BatchGetHandler::new(
            fx.hot.clone(),
            fx.ledger.clone(),
            std::time::Duration::from_millis(20),
        );
        let q = BatchGetQuery::new(vec![post("p1")], vec![Metric::View]).unwrap();
        let out = handler.handle(run(q)).await.unwrap();

        assert!(out.degraded);
        assert_eq!(out.snapshots[0].get(Metric::View), Some(777));
    }
}
