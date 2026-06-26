//! The cold time-series tier over ScyllaDB (TWCS).
//!
//! Window deltas are rolled up into coarse hour buckets via a `counter` column, so
//! a high-volume stream of fine windows collapses into a few additive cells per
//! entity/metric. The single range read backs `GetTimeSeries`; it is off the feed
//! path and uses the latency-relaxed read profile.
//!
//! Schema (owned by the `migrator`):
//! ```cql
//! CREATE TABLE counter.timeseries (
//!   entity_kind text, entity_id text, metric text,
//!   bucket_start timestamp,
//!   value counter,
//!   PRIMARY KEY ((entity_kind, entity_id, metric), bucket_start)
//! ) WITH compaction = { 'class': 'TimeWindowCompactionStrategy' };
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;
use scylla::value::{Counter, CqlTimestamp};
use scylla_storage::{ProfileKind, ScyllaClient, ScyllaStorageError};

use crate::domain::{TimeSeriesBucket, TimeSeriesQuery, WindowDelta, WindowSize};
use crate::error::CounterError;

const HOUR_MS: i64 = 3_600_000;

fn scylla_write_err(e: scylla::errors::ExecutionError) -> CounterError {
    CounterError::FlushFailed {
        reason: ScyllaStorageError::from(e).to_string(),
    }
}

fn scylla_read_err(_e: impl ToString) -> CounterError {
    CounterError::TimeSeriesUnavailable
}

/// Floor an epoch-millis instant to the start of its hour bucket.
fn hour_bucket(ms: i64) -> i64 {
    (ms.div_euclid(HOUR_MS)) * HOUR_MS
}

pub struct ScyllaTimeSeriesStore {
    client: Arc<ScyllaClient>,
    window_size: WindowSize,
}

impl ScyllaTimeSeriesStore {
    pub fn new(client: Arc<ScyllaClient>, window_size: WindowSize) -> Self {
        Self {
            client,
            window_size,
        }
    }

    fn stmt(&self, cql: &str, profile: ProfileKind) -> Statement {
        let mut s = Statement::new(cql);
        s.set_execution_profile_handle(Some(
            self.client
                .profiles
                .get(profile)
                .clone()
                .into_handle_with_label("counter-timeseries".to_string()),
        ));
        s.set_history_listener(Arc::clone(&self.client.history_listener) as Arc<dyn HistoryListener>);
        s
    }
}

#[async_trait]
impl crate::application::port::TimeSeriesStore for ScyllaTimeSeriesStore {
    async fn append(&self, delta: &WindowDelta) -> Result<(), CounterError> {
        let bucket = hour_bucket(delta.window().start_millis(self.window_size) as i64);
        let stmt = self.stmt(
            "UPDATE counter.timeseries SET value = value + ? \
             WHERE entity_kind = ? AND entity_id = ? AND metric = ? AND bucket_start = ?",
            ProfileKind::Strict,
        );
        self.client
            .session
            .execute_unpaged(
                stmt,
                (
                    // A counter increment must be bound as the `Counter` newtype —
                    // a bare i64 serializes as BigInt, which the prepared-statement
                    // type check rejects against a `counter` column.
                    Counter(delta.scalar()),
                    delta.entity().kind.as_str(),
                    delta.entity().id.as_str(),
                    delta.metric().as_str(),
                    CqlTimestamp(bucket),
                ),
            )
            .await
            .map_err(scylla_write_err)?;
        Ok(())
    }

    async fn range(&self, query: &TimeSeriesQuery) -> Result<Vec<TimeSeriesBucket>, CounterError> {
        let stmt = self.stmt(
            "SELECT bucket_start, value FROM counter.timeseries \
             WHERE entity_kind = ? AND entity_id = ? AND metric = ? \
             AND bucket_start >= ? AND bucket_start < ?",
            ProfileKind::Fast,
        );
        let rows = self
            .client
            .session
            .execute_unpaged(
                stmt,
                (
                    query.entity.kind.as_str(),
                    query.entity.id.as_str(),
                    query.metric.as_str(),
                    CqlTimestamp(query.start.timestamp_millis()),
                    CqlTimestamp(query.end.timestamp_millis()),
                ),
            )
            .await
            .map_err(scylla_read_err)?
            .into_rows_result()
            .map_err(scylla_read_err)?;

        let mut out = Vec::new();
        for row in rows
            .rows::<(CqlTimestamp, Counter)>()
            .map_err(scylla_read_err)?
        {
            let (ts, value) = row.map_err(scylla_read_err)?;
            if let Some(bucket_start) = chrono::DateTime::from_timestamp_millis(ts.0) {
                out.push(TimeSeriesBucket {
                    bucket_start,
                    value: value.0,
                });
            }
        }
        Ok(out)
    }
}
