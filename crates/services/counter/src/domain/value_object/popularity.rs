use serde::{Deserialize, Serialize};

use super::metric::Metric;

/// A coarse, non-negative popularity signal derived on a slow loop from counter
/// snapshots.
///
/// This is the value counter-analytics publishes on `counter.v1.popularity` and
/// that `search` projects into its `PopularityScore` ranking input (it currently
/// projects zero — this signal unblocks it). It is deliberately *coarse*: derived
/// periodically from aggregated magnitudes, never per-event, so it never causes
/// write-amplification downstream.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PopularityScore(f64);

impl PopularityScore {
    pub const ZERO: Self = Self(0.0);

    /// Clamp to non-negative; a negative popularity is meaningless.
    pub fn new(value: f64) -> Self {
        Self(if value.is_finite() { value.max(0.0) } else { 0.0 })
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl Default for PopularityScore {
    fn default() -> Self {
        PopularityScore::ZERO
    }
}

/// Per-metric weights blended into a [`PopularityScore`]. The defaults encode a
/// deliberate ordering: a deep engagement (share) counts for more than a cheap
/// one (a view), so virality outranks raw reach. Tunable without touching the
/// fold or the read path.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PopularityWeights {
    pub view: f64,
    pub like: f64,
    pub share: f64,
    pub comment: f64,
}

impl Default for PopularityWeights {
    fn default() -> Self {
        Self {
            view: 0.1,
            like: 1.0,
            share: 3.0,
            comment: 2.0,
        }
    }
}

impl PopularityWeights {
    /// The weight applied to a metric; metrics without a weight contribute nothing
    /// to the coarse score (followers, impressions, etc. are not popularity
    /// inputs by default).
    pub fn for_metric(&self, metric: Metric) -> f64 {
        match metric {
            Metric::View => self.view,
            Metric::Like => self.like,
            Metric::Share => self.share,
            Metric::Comment => self.comment,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_negative_and_non_finite_to_zero() {
        assert_eq!(PopularityScore::new(-3.0).value(), 0.0);
        assert_eq!(PopularityScore::new(f64::NAN).value(), 0.0);
        assert_eq!(PopularityScore::new(f64::INFINITY).value(), 0.0);
    }

    #[test]
    fn preserves_non_negative() {
        assert_eq!(PopularityScore::new(12.5).value(), 12.5);
    }

    #[test]
    fn default_weights_rank_share_above_view() {
        let w = PopularityWeights::default();
        assert!(w.for_metric(Metric::Share) > w.for_metric(Metric::View));
        assert_eq!(w.for_metric(Metric::Follower), 0.0);
    }
}
