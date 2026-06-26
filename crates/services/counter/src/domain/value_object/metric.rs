use serde::{Deserialize, Serialize};

use crate::error::CounterError;

/// Whether a served count is reconcilable against an authoritative set, or an
/// accepted estimate. This is the single most load-bearing classification in the
/// domain: it decides whether at-least-once double-counting is a bug or a
/// tolerated property, and whether the reconciliation loop (Phase 7) is even
/// applicable to the metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricKind {
    /// A window onto a set another service authoritatively owns (likes, follows).
    /// Served fast and approximate on the hot path, but **periodically reconciled**
    /// to exactness against the source-of-record. Double-counting here is a defect.
    Exact,
    /// Firehose telemetry no one audits per-unit (views, unique viewers). Served
    /// from sharded counters / HyperLogLog within a documented error bound;
    /// at-least-once double-counting is **tolerated by design** and never
    /// reconciled per-unit.
    Approximate,
}

/// How a metric is aggregated. Orthogonal to [`MetricKind`]: a `Sum` metric may be
/// `Exact` (likes) or `Approximate` (views); a `Cardinality` metric is always
/// `Approximate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aggregation {
    /// Signed additive magnitude (`+1` like, `-1` unlike, `+1/-1` follow). Folded
    /// by summing deltas; served from a counter (`INCRBY` / sharded `INCRBY`).
    Sum,
    /// Distinct-element count over actor/viewer ids. Folded by collecting members;
    /// served from a HyperLogLog (`PFADD` / `PFCOUNT`). Requires a member id per
    /// observation.
    Cardinality,
}

/// The metric being counted across the fleet. The exact-vs-approximate split and
/// the aggregation strategy are intrinsic properties of the metric, decided here
/// once so every layer (fold, store adapter, reconciliation, read) agrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metric {
    View,
    Impression,
    Click,
    Like,
    Share,
    Comment,
    Follower,
    Following,
    UniqueViewer,
    Reach,
}

impl Metric {
    /// Every metric, for exhaustive iteration in tests and "all metrics" reads.
    pub const ALL: [Metric; 10] = [
        Metric::View,
        Metric::Impression,
        Metric::Click,
        Metric::Like,
        Metric::Share,
        Metric::Comment,
        Metric::Follower,
        Metric::Following,
        Metric::UniqueViewer,
        Metric::Reach,
    ];

    /// Stable lowercase discriminant used in keys, ledger rows, and event mapping.
    pub fn as_str(&self) -> &'static str {
        match self {
            Metric::View => "view",
            Metric::Impression => "impression",
            Metric::Click => "click",
            Metric::Like => "like",
            Metric::Share => "share",
            Metric::Comment => "comment",
            Metric::Follower => "follower",
            Metric::Following => "following",
            Metric::UniqueViewer => "unique_viewer",
            Metric::Reach => "reach",
        }
    }

    /// Parse the stable discriminant. An unrecognized value is surfaced as
    /// `CTR-1002 UnsupportedMetric`.
    pub fn try_from_str(s: &str) -> Result<Self, CounterError> {
        Metric::ALL
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| CounterError::UnsupportedMetric {
                metric: s.to_owned(),
            })
    }

    /// Reconcilable (`Exact`) vs accepted estimate (`Approximate`).
    pub fn kind(&self) -> MetricKind {
        match self {
            // Windows onto sets owned by `engagement` / `social-graph` / `comment`.
            Metric::Like
            | Metric::Share
            | Metric::Comment
            | Metric::Follower
            | Metric::Following => MetricKind::Exact,
            // Firehose telemetry, accepted approximate.
            Metric::View
            | Metric::Impression
            | Metric::Click
            | Metric::UniqueViewer
            | Metric::Reach => MetricKind::Approximate,
        }
    }

    /// Additive sum vs distinct-element cardinality.
    pub fn aggregation(&self) -> Aggregation {
        match self {
            Metric::UniqueViewer | Metric::Reach => Aggregation::Cardinality,
            _ => Aggregation::Sum,
        }
    }

    /// Whether folding an observation of this metric requires a unique member id
    /// (true for cardinality metrics, which estimate distinct actors).
    pub fn requires_member(&self) -> bool {
        matches!(self.aggregation(), Aggregation::Cardinality)
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn all_metrics_round_trip() {
        for m in Metric::ALL {
            assert_eq!(Metric::try_from_str(m.as_str()).unwrap(), m);
        }
    }

    #[test]
    fn rejects_unknown_metric() {
        let err = Metric::try_from_str("dwell_time").unwrap_err();
        assert_eq!(err.error_code(), "CTR-1002");
    }

    #[test]
    fn cardinality_metrics_are_always_approximate() {
        for m in Metric::ALL {
            if m.aggregation() == Aggregation::Cardinality {
                assert_eq!(m.kind(), MetricKind::Approximate, "{m:?}");
                assert!(m.requires_member(), "{m:?}");
            }
        }
    }

    #[test]
    fn exact_metrics_are_reconcilable_sums() {
        for m in [
            Metric::Like,
            Metric::Share,
            Metric::Comment,
            Metric::Follower,
            Metric::Following,
        ] {
            assert_eq!(m.kind(), MetricKind::Exact);
            assert_eq!(m.aggregation(), Aggregation::Sum);
            assert!(!m.requires_member());
        }
    }

    #[test]
    fn views_are_approximate_sums() {
        assert_eq!(Metric::View.kind(), MetricKind::Approximate);
        assert_eq!(Metric::View.aggregation(), Aggregation::Sum);
    }
}
