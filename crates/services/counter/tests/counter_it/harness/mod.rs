//! Integration harness: boots ephemeral Redis + Postgres + Scylla, applies the
//! `.sql` / `.cql` migrations, and wires the real counter adapters against them.
//! Isolation is by fresh per-scenario entity ids (UUID) — the shared containers
//! run every scenario in parallel.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use async_trait::async_trait;
use std::collections::HashMap;

use counter::application::command::{DeltaFlusher, FlushReport, Reconciler};
use counter::application::port::{
    CounterLedger, CounterStore, ReconciliationSource, TimeSeriesStore,
};
use counter::domain::{
    CountSnapshot, EntityId, EntityKind, EntityRef, Metric, Observation, TimeSeriesBucket,
    TimeSeriesQuery, TrendingItem, TrendingScope, WindowAggregator, WindowDelta, WindowSize,
};
use counter::infrastructure::pg_counter_ledger::PgCounterLedger;
use counter::infrastructure::redis_counter_store::RedisCounterStore;
use counter::infrastructure::scylla_time_series::ScyllaTimeSeriesStore;

use postgres_storage::config::StatementLogLevel;
use postgres_storage::{PgPoolBuilder, PostgresConfig, TransactionManager};
use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};

const PG_MIGRATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/postgres");
const SCYLLA_MIGRATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/scylla");
const WINDOW_MS: u64 = 5_000;

pub struct Harness {
    pub store: Arc<dyn CounterStore>,
    pub ledger: Arc<dyn CounterLedger>,
    pub series: Arc<dyn TimeSeriesStore>,
    flusher: DeltaFlusher,
    window: WindowSize,
}

impl Harness {
    pub async fn start() -> Self {
        let pg_url = test_support::containers::postgres_ready(PG_MIGRATIONS).await;
        let redis_endpoint = test_support::containers::redis_endpoint().await;
        let scylla_cp = test_support::containers::scylla_ready("counter", SCYLLA_MIGRATIONS).await;

        let pg_config = PostgresConfig {
            database_url: pg_url,
            max_connections: 8,
            min_connections: 1,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: None,
            max_lifetime: None,
            statement_log_level: StatementLogLevel::Debug,
            slow_statement_threshold: Duration::from_millis(500),
        };
        let pool = PgPoolBuilder::build(pg_config).await.expect("it: pg pool");
        let tx = TransactionManager::new(pool);

        let redis = RedisClientBuilder::new(RedisConfig {
            hosts: vec![redis_endpoint],
            ..RedisConfig::default()
        })
        .build()
        .await
        .expect("it: redis client");

        let scylla = Arc::new(
            ScyllaSessionBuilder::new(ScyllaConfig {
                contact_points: vec![scylla_cp],
                keyspace: None,
                ..ScyllaConfig::default()
            })
            .build()
            .await
            .expect("it: scylla client"),
        );

        let window = WindowSize::from_millis(WINDOW_MS).unwrap();
        let store: Arc<dyn CounterStore> = Arc::new(RedisCounterStore::new(redis));
        let ledger: Arc<dyn CounterLedger> = Arc::new(PgCounterLedger::new(tx));
        let series: Arc<dyn TimeSeriesStore> =
            Arc::new(ScyllaTimeSeriesStore::new(scylla, window));
        let flusher = DeltaFlusher::new(Arc::clone(&store), Arc::clone(&ledger), Arc::clone(&series));

        Self {
            store,
            ledger,
            series,
            flusher,
            window,
        }
    }

    pub fn window(&self) -> WindowSize {
        self.window
    }

    /// Fold observations into a fresh window aggregator, drain everything, and flush
    /// across the three tiers. Returns the flush report.
    pub async fn ingest(&self, observations: Vec<Observation>) -> FlushReport {
        let deltas = self.fold(observations);
        self.flusher.flush(&deltas).await.expect("it: flush")
    }

    /// Fold observations into deltas without flushing (so a scenario can flush the
    /// same deltas twice to prove idempotency).
    pub fn fold(&self, observations: Vec<Observation>) -> Vec<WindowDelta> {
        let mut agg = WindowAggregator::new(self.window);
        for o in observations {
            agg.fold(o).expect("it: fold");
        }
        agg.drain_all()
    }

    pub async fn flush(&self, deltas: &[WindowDelta]) -> FlushReport {
        self.flusher.flush(deltas).await.expect("it: flush")
    }

    pub async fn read(&self, entity: &EntityRef, metrics: &[Metric]) -> CountSnapshot {
        self.store
            .read(std::slice::from_ref(entity), metrics)
            .await
            .expect("it: read")
            .pop()
            .expect("it: one snapshot")
    }

    pub async fn total(&self, entity: &EntityRef, metric: Metric) -> Option<i64> {
        self.ledger.read_total(entity, metric).await.expect("it: total")
    }

    pub async fn top_k(&self, metric: Metric, limit: usize) -> Vec<TrendingItem> {
        self.store
            .top_k(TrendingScope::Global, None, metric, limit)
            .await
            .expect("it: top_k")
    }

    pub async fn range(&self, query: &TimeSeriesQuery) -> Vec<TimeSeriesBucket> {
        self.series.range(query).await.expect("it: range")
    }

    /// A reconciler over the live store + ledger and a caller-supplied authoritative
    /// source.
    pub fn reconciler(&self, source: Arc<dyn ReconciliationSource>, tolerance: i64) -> Reconciler {
        Reconciler::new(
            Arc::clone(&self.store),
            Arc::clone(&self.ledger),
            source,
            tolerance,
        )
    }
}

/// An authoritative-count source whose values the scenario sets — stands in for the
/// gRPC call to engagement / social-graph.
#[derive(Default)]
pub struct FixedSource {
    counts: std::sync::Mutex<HashMap<(String, String, String), i64>>,
}

impl FixedSource {
    pub fn set(&self, entity: &EntityRef, metric: Metric, value: i64) {
        self.counts.lock().unwrap().insert(
            (
                entity.kind.as_str().to_owned(),
                entity.id.as_str().to_owned(),
                metric.as_str().to_owned(),
            ),
            value,
        );
    }
}

#[async_trait]
impl ReconciliationSource for FixedSource {
    async fn authoritative_count(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, counter::CounterError> {
        Ok(self
            .counts
            .lock()
            .unwrap()
            .get(&(
                entity.kind.as_str().to_owned(),
                entity.id.as_str().to_owned(),
                metric.as_str().to_owned(),
            ))
            .copied())
    }
}

// ── Builders ──────────────────────────────────────────────────────────────────

/// A fresh, scenario-isolated post entity.
pub fn fresh_post() -> EntityRef {
    EntityRef::new(
        EntityKind::Post,
        EntityId::new(format!("post-{}", Uuid::now_v7())).unwrap(),
    )
}

pub fn at(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).unwrap()
}

pub fn view(entity: &EntityRef, ms: i64) -> Observation {
    Observation::sum(entity.clone(), Metric::View, 1, at(ms)).unwrap()
}

pub fn unique_view(entity: &EntityRef, member: &str, ms: i64) -> Observation {
    use counter::domain::MemberId;
    Observation::unique(
        entity.clone(),
        Metric::UniqueViewer,
        MemberId::new(member).unwrap(),
        at(ms),
    )
    .unwrap()
}
