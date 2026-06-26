//! In-memory fakes for the four ports, plus a [`Fixture`] composition root, for
//! the application unit tests. They model the semantics that matter — the durable
//! ledger's `(entity, metric, window_id)` idempotency, the hot tier's sum/HLL
//! split, the fail-open `unavailable` toggle — without any container.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use crate::application::command::{DeltaFlusher, PopularityPublisher, Reconciler};
use crate::application::port::{
    CounterLedger, CounterStore, FlushOutcome, ReconciliationSource, SignalPublisher,
    TimeSeriesStore,
};
use crate::application::query::{BatchGetHandler, TimeSeriesHandler, TrendingHandler};
use crate::domain::{
    Aggregation, CountSnapshot, CounterValue, EntityRef, Metric, PopularityScore, TimeSeriesBucket,
    TimeSeriesQuery, TrendingItem, TrendingScope, WindowDelta,
};
use crate::error::CounterError;

type MetricKey = (String, String, String);
type EntityKey = (String, String);
/// Per-metric trending board: entity key → (entity, accumulated score).
type TrendingBoard = HashMap<String, HashMap<EntityKey, (EntityRef, i64)>>;
/// Per-`(entity, metric)` time-series: a list of `(bucket_start, value)` points.
type SeriesStore = HashMap<MetricKey, Vec<(DateTime<Utc>, i64)>>;

fn mkey(entity: &EntityRef, metric: Metric) -> MetricKey {
    (
        entity.kind.as_str().to_owned(),
        entity.id.as_str().to_owned(),
        metric.as_str().to_owned(),
    )
}

fn ekey(entity: &EntityRef) -> EntityKey {
    (
        entity.kind.as_str().to_owned(),
        entity.id.as_str().to_owned(),
    )
}

// ── Hot tier (Redis analogue) ─────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryHotStore {
    sums: Mutex<HashMap<MetricKey, i64>>,
    hll: Mutex<HashMap<MetricKey, BTreeSet<String>>>,
    trending: Mutex<TrendingBoard>,
    unavailable: AtomicBool,
    /// Artificial read latency, in millis, to exercise the read-path timeout.
    read_delay_ms: AtomicU64,
}

impl InMemoryHotStore {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    /// Make `read` sleep this long, to trip the handler's hard timeout.
    pub fn set_read_delay(&self, delay: Duration) {
        self.read_delay_ms
            .store(delay.as_millis() as u64, Ordering::SeqCst);
    }

    fn guard(&self) -> Result<(), CounterError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(CounterError::HotStoreUnavailable);
        }
        Ok(())
    }

    pub fn seed(&self, entity: &EntityRef, metric: Metric, value: i64) {
        self.sums.lock().unwrap().insert(mkey(entity, metric), value);
    }

    pub fn seed_trending(&self, metric: Metric, entity: &EntityRef, score: i64) {
        self.trending
            .lock()
            .unwrap()
            .entry(metric.as_str().to_owned())
            .or_default()
            .insert(ekey(entity), (entity.clone(), score));
    }

    /// The current value of a metric (sum counter or HLL cardinality), for asserts.
    pub fn value(&self, entity: &EntityRef, metric: Metric) -> i64 {
        match metric.aggregation() {
            Aggregation::Sum => self
                .sums
                .lock()
                .unwrap()
                .get(&mkey(entity, metric))
                .copied()
                .unwrap_or(0),
            Aggregation::Cardinality => self
                .hll
                .lock()
                .unwrap()
                .get(&mkey(entity, metric))
                .map(|s| s.len() as i64)
                .unwrap_or(0),
        }
    }
}

#[async_trait]
impl CounterStore for InMemoryHotStore {
    async fn apply_delta(&self, delta: &WindowDelta) -> Result<(), CounterError> {
        self.guard()?;
        let metric = delta.metric();
        let key = mkey(delta.entity(), metric);
        match metric.aggregation() {
            Aggregation::Sum => {
                *self.sums.lock().unwrap().entry(key).or_insert(0) += delta.sum;
                let mut t = self.trending.lock().unwrap();
                let bucket = t.entry(metric.as_str().to_owned()).or_default();
                let entry = bucket
                    .entry(ekey(delta.entity()))
                    .or_insert_with(|| (delta.entity().clone(), 0));
                entry.1 += delta.sum;
            }
            Aggregation::Cardinality => {
                let mut hll = self.hll.lock().unwrap();
                let set = hll.entry(key).or_default();
                for m in &delta.unique_members {
                    set.insert(m.as_str().to_owned());
                }
            }
        }
        Ok(())
    }

    async fn read(
        &self,
        entities: &[EntityRef],
        metrics: &[Metric],
    ) -> Result<Vec<CountSnapshot>, CounterError> {
        let delay = self.read_delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        self.guard()?;
        let sums = self.sums.lock().unwrap();
        let hll = self.hll.lock().unwrap();
        let mut out = Vec::with_capacity(entities.len());
        for entity in entities {
            let mut values = Vec::new();
            for &metric in metrics {
                let key = mkey(entity, metric);
                let v = match metric.aggregation() {
                    Aggregation::Sum => sums.get(&key).copied(),
                    Aggregation::Cardinality => hll.get(&key).map(|s| s.len() as i64),
                };
                if let Some(v) = v {
                    values.push(CounterValue::new(metric, v));
                }
            }
            out.push(CountSnapshot::new(entity.clone(), values));
        }
        Ok(out)
    }

    async fn top_k(
        &self,
        scope: TrendingScope,
        _scope_key: Option<&str>,
        metric: Metric,
        limit: usize,
    ) -> Result<Vec<TrendingItem>, CounterError> {
        self.guard()?;
        if scope != TrendingScope::Global {
            return Ok(Vec::new()); // scoped trending not modeled in the fake
        }
        let trending = self.trending.lock().unwrap();
        let mut ranked: Vec<(EntityRef, i64)> = trending
            .get(metric.as_str())
            .map(|b| b.values().cloned().collect())
            .unwrap_or_default();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(ranked
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, (entity, score))| TrendingItem {
                entity,
                score,
                rank: i as u32,
            })
            .collect())
    }

    async fn overwrite(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError> {
        self.guard()?;
        self.sums.lock().unwrap().insert(mkey(entity, metric), value);
        Ok(())
    }
}

// ── Warm tier (Postgres ledger analogue) ──────────────────────────────────────

#[derive(Default)]
pub struct InMemoryLedger {
    totals: Mutex<HashMap<MetricKey, i64>>,
    applied: Mutex<HashSet<(String, String, String, u64)>>,
    unavailable: AtomicBool,
}

impl InMemoryLedger {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    fn guard(&self) -> Result<(), CounterError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(CounterError::LedgerUnavailable);
        }
        Ok(())
    }

    pub fn seed_total(&self, entity: &EntityRef, metric: Metric, value: i64) {
        self.totals
            .lock()
            .unwrap()
            .insert(mkey(entity, metric), value);
    }

    pub fn total(&self, entity: &EntityRef, metric: Metric) -> Option<i64> {
        self.totals.lock().unwrap().get(&mkey(entity, metric)).copied()
    }
}

#[async_trait]
impl CounterLedger for InMemoryLedger {
    async fn flush_window(&self, delta: &WindowDelta) -> Result<FlushOutcome, CounterError> {
        self.guard()?;
        let (k0, k1, k2) = mkey(delta.entity(), delta.metric());
        let window_key = (k0.clone(), k1.clone(), k2.clone(), delta.window().index());
        let mut applied = self.applied.lock().unwrap();
        if !applied.insert(window_key) {
            return Ok(FlushOutcome::AlreadyApplied);
        }
        *self.totals.lock().unwrap().entry((k0, k1, k2)).or_insert(0) += delta.scalar();
        Ok(FlushOutcome::Applied)
    }

    async fn read_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError> {
        self.guard()?;
        Ok(self.total(entity, metric))
    }

    async fn set_total(
        &self,
        entity: &EntityRef,
        metric: Metric,
        value: i64,
    ) -> Result<(), CounterError> {
        self.guard()?;
        self.totals
            .lock()
            .unwrap()
            .insert(mkey(entity, metric), value);
        Ok(())
    }
}

// ── Cold tier (Scylla time-series analogue) ───────────────────────────────────

#[derive(Default)]
pub struct InMemoryTimeSeries {
    buckets: Mutex<SeriesStore>,
    unavailable: AtomicBool,
}

impl InMemoryTimeSeries {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    fn guard(&self) -> Result<(), CounterError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(CounterError::TimeSeriesUnavailable);
        }
        Ok(())
    }

    pub fn seed_bucket(&self, entity: &EntityRef, metric: Metric, start: DateTime<Utc>, value: i64) {
        self.buckets
            .lock()
            .unwrap()
            .entry(mkey(entity, metric))
            .or_default()
            .push((start, value));
    }

    pub fn bucket_count(&self, entity: &EntityRef, metric: Metric) -> usize {
        self.buckets
            .lock()
            .unwrap()
            .get(&mkey(entity, metric))
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[async_trait]
impl TimeSeriesStore for InMemoryTimeSeries {
    async fn append(&self, delta: &WindowDelta) -> Result<(), CounterError> {
        self.guard()?;
        // Synthesize a deterministic bucket instant from the window index.
        let start = Utc.timestamp_millis_opt(delta.window().index() as i64).unwrap();
        self.seed_bucket(delta.entity(), delta.metric(), start, delta.scalar());
        Ok(())
    }

    async fn range(&self, query: &TimeSeriesQuery) -> Result<Vec<TimeSeriesBucket>, CounterError> {
        self.guard()?;
        let buckets = self.buckets.lock().unwrap();
        let mut out: Vec<TimeSeriesBucket> = buckets
            .get(&mkey(&query.entity, query.metric))
            .map(|series| {
                series
                    .iter()
                    .filter(|(ts, _)| *ts >= query.start && *ts < query.end)
                    .map(|(ts, v)| TimeSeriesBucket {
                        bucket_start: *ts,
                        value: *v,
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|b| b.bucket_start);
        Ok(out)
    }
}

// ── Signal publisher (Kafka analogue) ─────────────────────────────────────────

#[derive(Default)]
pub struct InMemorySignalPublisher {
    scores: Mutex<HashMap<EntityKey, f64>>,
}

impl InMemorySignalPublisher {
    pub fn last_score(&self, entity: &EntityRef) -> Option<f64> {
        self.scores.lock().unwrap().get(&ekey(entity)).copied()
    }
}

#[async_trait]
impl SignalPublisher for InMemorySignalPublisher {
    async fn publish_popularity(
        &self,
        entity: &EntityRef,
        score: PopularityScore,
    ) -> Result<(), CounterError> {
        self.scores
            .lock()
            .unwrap()
            .insert(ekey(entity), score.value());
        Ok(())
    }
}

// ── Reconciliation source (engagement / social-graph analogue) ────────────────

#[derive(Default)]
pub struct InMemoryReconciliationSource {
    authoritative: Mutex<HashMap<MetricKey, i64>>,
}

impl InMemoryReconciliationSource {
    pub fn set_authoritative(&self, entity: &EntityRef, metric: Metric, value: i64) {
        self.authoritative
            .lock()
            .unwrap()
            .insert(mkey(entity, metric), value);
    }
}

#[async_trait]
impl ReconciliationSource for InMemoryReconciliationSource {
    async fn authoritative_count(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError> {
        Ok(self
            .authoritative
            .lock()
            .unwrap()
            .get(&mkey(entity, metric))
            .copied())
    }
}

// ── Composition root ──────────────────────────────────────────────────────────

/// Holds the concrete fakes (so tests can seed + assert) and builds the
/// application services over them as `Arc<dyn …>`.
pub struct Fixture {
    pub hot: std::sync::Arc<InMemoryHotStore>,
    pub ledger: std::sync::Arc<InMemoryLedger>,
    pub series: std::sync::Arc<InMemoryTimeSeries>,
    pub publisher: std::sync::Arc<InMemorySignalPublisher>,
    pub source: std::sync::Arc<InMemoryReconciliationSource>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            hot: std::sync::Arc::new(InMemoryHotStore::default()),
            ledger: std::sync::Arc::new(InMemoryLedger::default()),
            series: std::sync::Arc::new(InMemoryTimeSeries::default()),
            publisher: std::sync::Arc::new(InMemorySignalPublisher::default()),
            source: std::sync::Arc::new(InMemoryReconciliationSource::default()),
        }
    }

    pub fn delta_flusher(&self) -> DeltaFlusher {
        DeltaFlusher::new(self.hot.clone(), self.ledger.clone(), self.series.clone())
    }

    pub fn popularity_publisher(&self) -> PopularityPublisher {
        PopularityPublisher::new(self.hot.clone(), self.publisher.clone())
    }

    pub fn reconciler(&self, tolerance: i64) -> Reconciler {
        Reconciler::new(
            self.hot.clone(),
            self.ledger.clone(),
            self.source.clone(),
            tolerance,
        )
    }

    /// A generous default read timeout that never trips in tests.
    pub fn batch_get_handler(&self) -> BatchGetHandler {
        BatchGetHandler::new(
            self.hot.clone(),
            self.ledger.clone(),
            std::time::Duration::from_secs(5),
        )
    }

    pub fn trending_handler(&self) -> TrendingHandler {
        TrendingHandler::new(self.hot.clone())
    }

    pub fn time_series_handler(&self) -> TimeSeriesHandler {
        TimeSeriesHandler::new(self.series.clone())
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}
