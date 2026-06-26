//! The counter-analytics composition roots.
//!
//! [`compose_read`] is *pure* wiring (the storage ports in, the gRPC read handler
//! out — no I/O), so the unit/integration graph and the binary build the exact same
//! handler over the fakes or the real adapters. [`Ports::build`] is the I/O variant
//! that constructs the three storage adapters from config; both binaries build from
//! it (the read server uses the ports for the query handlers, the worker additionally
//! builds the Kafka producer + the write-side handlers).

use std::sync::Arc;
use std::time::Duration;

use postgres_storage::{PgPoolBuilder, PostgresConfig, TransactionManager};
use redis_storage::{RedisClient, RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};

use crate::application::port::{CounterLedger, CounterStore, TimeSeriesStore};
use crate::application::query::{BatchGetHandler, TimeSeriesHandler, TrendingHandler};
use crate::domain::WindowSize;
use crate::infrastructure::grpc::CounterServiceHandler;
use crate::infrastructure::pg_counter_ledger::PgCounterLedger;
use crate::infrastructure::redis_counter_store::RedisCounterStore;
use crate::infrastructure::scylla_time_series::ScyllaTimeSeriesStore;

/// The shared storage adapter set both binaries build from. Retains the concrete
/// [`RedisClient`] so the runtime can build a hot-tier liveness probe.
pub struct Ports {
    pub store: Arc<dyn CounterStore>,
    pub ledger: Arc<dyn CounterLedger>,
    pub series: Arc<dyn TimeSeriesStore>,
    pub redis: RedisClient,
}

impl Ports {
    /// Connects the three storage tiers and wraps them as ports. Connections are
    /// established eagerly; a tier being briefly down at boot surfaces as an error
    /// the runtime supervises.
    pub async fn build(
        postgres: PostgresConfig,
        redis: RedisConfig,
        scylla: ScyllaConfig,
        window: WindowSize,
    ) -> Result<Ports, Box<dyn std::error::Error>> {
        let pool = PgPoolBuilder::build(postgres).await?;
        let tx = TransactionManager::new(pool);
        let redis = RedisClientBuilder::new(redis).build().await?;
        let scylla = Arc::new(ScyllaSessionBuilder::new(scylla).build().await?);

        let store: Arc<dyn CounterStore> = Arc::new(RedisCounterStore::new(redis.clone()));
        let ledger: Arc<dyn CounterLedger> = Arc::new(PgCounterLedger::new(tx));
        let series: Arc<dyn TimeSeriesStore> =
            Arc::new(ScyllaTimeSeriesStore::new(scylla, window));

        Ok(Ports {
            store,
            ledger,
            series,
            redis,
        })
    }
}

/// Pure read composition: the three query handlers wrapped in the gRPC handler.
/// Drives the unit/integration graph over the fakes; the binary calls it over the
/// real adapters.
pub fn compose_read(
    store: Arc<dyn CounterStore>,
    ledger: Arc<dyn CounterLedger>,
    series: Arc<dyn TimeSeriesStore>,
    read_timeout: Duration,
) -> CounterServiceHandler {
    let batch = Arc::new(BatchGetHandler::new(
        Arc::clone(&store),
        Arc::clone(&ledger),
        read_timeout,
    ));
    let trending = Arc::new(TrendingHandler::new(Arc::clone(&store)));
    let timeseries = Arc::new(TimeSeriesHandler::new(series));
    CounterServiceHandler::new(batch, trending, timeseries)
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::{EntityId, EntityKind, EntityRef, Metric};
    use crate::infrastructure::grpc::proto;

    fn post(id: &str) -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new(id).unwrap())
    }

    #[tokio::test]
    async fn batch_get_rpc_returns_counts() {
        let fx = Fixture::new();
        fx.hot.seed(&post("p1"), Metric::View, 99);

        let handler = compose_read(
            fx.hot.clone(),
            fx.ledger.clone(),
            fx.series.clone(),
            std::time::Duration::from_secs(5),
        );
        let request = Request::new(proto::BatchGetCountersRequest {
            entities: vec![proto::EntityRef {
                entity_type: proto::CounterEntityType::Post as i32,
                id: "p1".into(),
            }],
            metrics: vec![proto::CounterMetric::View as i32],
        });
        let resp = handler.batch_get_counters(request).await.unwrap().into_inner();

        assert!(!resp.degraded);
        assert_eq!(resp.snapshots.len(), 1);
        assert_eq!(resp.snapshots[0].values[0].value, 99);
        assert_eq!(
            resp.snapshots[0].values[0].metric,
            proto::CounterMetric::View as i32
        );
    }

    #[tokio::test]
    async fn batch_get_rpc_rejects_empty_entities() {
        let fx = Fixture::new();
        let handler = compose_read(
            fx.hot.clone(),
            fx.ledger.clone(),
            fx.series.clone(),
            std::time::Duration::from_secs(5),
        );
        let request = Request::new(proto::BatchGetCountersRequest {
            entities: vec![],
            metrics: vec![],
        });
        let status = handler.batch_get_counters(request).await.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    // Ensures the query handler is reachable through the trait impl (used by tonic).
    #[tokio::test]
    async fn trending_rpc_through_handler() {
        let fx = Fixture::new();
        fx.hot.seed_trending(Metric::View, &post("hot"), 500);
        let handler = compose_read(
            fx.hot.clone(),
            fx.ledger.clone(),
            fx.series.clone(),
            std::time::Duration::from_secs(5),
        );
        let request = Request::new(proto::GetTrendingRequest {
            scope: proto::TrendingScope::Global as i32,
            scope_key: String::new(),
            metric: proto::CounterMetric::View as i32,
            limit: 10,
        });
        let resp = handler.get_trending(request).await.unwrap().into_inner();
        assert_eq!(resp.entries.len(), 1);
        assert_eq!(resp.entries[0].entity.as_ref().unwrap().id, "hot");
    }
}
