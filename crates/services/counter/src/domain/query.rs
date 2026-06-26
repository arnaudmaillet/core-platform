//! Validated read inputs and result shapes for the query path. Mirrors the proto
//! surface (Phase 1) but is the domain's own vocabulary — proto ↔ domain mapping
//! lives at the edge (Phase 5), so these never depend on the generated types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::read::CountSnapshot;
use crate::domain::value_object::{EntityRef, Metric};
use crate::error::CounterError;

/// Hard ceiling on a single `BatchGetCounters` request, so one call can never fan
/// a feed page into an unbounded multi-get.
pub const MAX_BATCH: usize = 256;

/// Hard ceiling on a trending result; the server clamps `limit` to this.
pub const MAX_TRENDING: usize = 100;

/// A validated batch counter read. Empty or oversized batches are rejected as
/// `CTR-1001 InvalidCounterQuery`.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchGetQuery {
    pub entities: Vec<EntityRef>,
    /// Restrict to these metrics; empty ⇒ all metrics for each entity kind.
    pub metrics: Vec<Metric>,
}

impl BatchGetQuery {
    pub fn new(entities: Vec<EntityRef>, metrics: Vec<Metric>) -> Result<Self, CounterError> {
        if entities.is_empty() {
            return Err(CounterError::InvalidCounterQuery {
                reason: "at least one entity is required".to_owned(),
            });
        }
        if entities.len() > MAX_BATCH {
            return Err(CounterError::InvalidCounterQuery {
                reason: format!("batch of {} exceeds max {MAX_BATCH}", entities.len()),
            });
        }
        Ok(Self { entities, metrics })
    }

    /// The metrics to serve for an entity: the requested subset, or all metrics
    /// when none were specified.
    pub fn effective_metrics(&self) -> Vec<Metric> {
        if self.metrics.is_empty() {
            Metric::ALL.to_vec()
        } else {
            self.metrics.clone()
        }
    }
}

/// The result of a batch counter read: one snapshot per requested entity, plus a
/// fail-open marker.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchReadout {
    pub snapshots: Vec<CountSnapshot>,
    /// True when the hot tier was unavailable and these snapshots were served from
    /// the warm-ledger fallback (stale-but-served), or are otherwise degraded. The
    /// read never errors the feed — it degrades.
    pub degraded: bool,
}

/// The aggregation scope for a trending query. `Global` ranks across the network;
/// the others rank within a `scope_key` qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendingScope {
    Global,
    Hashtag,
    Category,
    Region,
}

impl TrendingScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrendingScope::Global => "global",
            TrendingScope::Hashtag => "hashtag",
            TrendingScope::Category => "category",
            TrendingScope::Region => "region",
        }
    }

    /// Whether this scope requires a non-empty `scope_key`.
    pub fn needs_key(&self) -> bool {
        !matches!(self, TrendingScope::Global)
    }
}

/// A validated trending query. A non-global scope without a key is rejected as
/// `CTR-1004 InvalidTrendingScope`; `limit` is clamped to `[1, MAX_TRENDING]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrendingQuery {
    pub scope: TrendingScope,
    pub scope_key: Option<String>,
    pub metric: Metric,
    pub limit: usize,
}

impl TrendingQuery {
    pub fn new(
        scope: TrendingScope,
        scope_key: Option<String>,
        metric: Metric,
        limit: usize,
    ) -> Result<Self, CounterError> {
        let scope_key = scope_key.filter(|k| !k.trim().is_empty());
        if scope.needs_key() && scope_key.is_none() {
            return Err(CounterError::InvalidTrendingScope {
                scope: scope.as_str().to_owned(),
            });
        }
        let limit = limit.clamp(1, MAX_TRENDING);
        Ok(Self {
            scope,
            scope_key,
            metric,
            limit,
        })
    }
}

/// One ranked trending result — an approximate Count-Min-Sketch score, never an
/// exact count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendingItem {
    pub entity: EntityRef,
    pub score: i64,
    pub rank: u32,
}

/// Bucket size for a historical time-series query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeGranularity {
    Hour,
    Day,
    Week,
}

impl TimeGranularity {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeGranularity::Hour => "hour",
            TimeGranularity::Day => "day",
            TimeGranularity::Week => "week",
        }
    }
}

/// A validated time-series query. An empty or inverted range is rejected as
/// `CTR-1003 InvalidTimeRange`.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSeriesQuery {
    pub entity: EntityRef,
    pub metric: Metric,
    pub granularity: TimeGranularity,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeSeriesQuery {
    pub fn new(
        entity: EntityRef,
        metric: Metric,
        granularity: TimeGranularity,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Self, CounterError> {
        if end <= start {
            return Err(CounterError::InvalidTimeRange {
                reason: "end must be strictly after start".to_owned(),
            });
        }
        Ok(Self {
            entity,
            metric,
            granularity,
            start,
            end,
        })
    }
}

/// One historical bucket: its start instant and the metric value over the bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSeriesBucket {
    pub bucket_start: DateTime<Utc>,
    pub value: i64,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;
    use crate::domain::value_object::{EntityId, EntityKind};

    fn post() -> EntityRef {
        EntityRef::new(EntityKind::Post, EntityId::new("p1").unwrap())
    }

    fn at(s: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(s, 0).unwrap()
    }

    #[test]
    fn batch_rejects_empty() {
        let err = BatchGetQuery::new(vec![], vec![]).unwrap_err();
        assert_eq!(err.error_code(), "CTR-1001");
    }

    #[test]
    fn batch_empty_metrics_means_all() {
        let q = BatchGetQuery::new(vec![post()], vec![]).unwrap();
        assert_eq!(q.effective_metrics().len(), Metric::ALL.len());
    }

    #[test]
    fn trending_global_needs_no_key() {
        let q = TrendingQuery::new(TrendingScope::Global, None, Metric::View, 10).unwrap();
        assert_eq!(q.limit, 10);
    }

    #[test]
    fn trending_scoped_requires_key() {
        let err =
            TrendingQuery::new(TrendingScope::Hashtag, Some("  ".to_owned()), Metric::View, 10)
                .unwrap_err();
        assert_eq!(err.error_code(), "CTR-1004");
    }

    #[test]
    fn trending_limit_is_clamped() {
        let q = TrendingQuery::new(TrendingScope::Global, None, Metric::View, 9_999).unwrap();
        assert_eq!(q.limit, MAX_TRENDING);
        let q = TrendingQuery::new(TrendingScope::Global, None, Metric::View, 0).unwrap();
        assert_eq!(q.limit, 1);
    }

    #[test]
    fn time_series_rejects_inverted_range() {
        let err = TimeSeriesQuery::new(
            post(),
            Metric::View,
            TimeGranularity::Day,
            at(100),
            at(100),
        )
        .unwrap_err();
        assert_eq!(err.error_code(), "CTR-1003");
    }
}
