use serde::{Deserialize, Serialize};

/// A coarse, periodically-refreshed popularity signal used as a ranking nudge.
///
/// Per the locked decision, search does **not** index real-time engagement counts
/// (that would melt the consumer and the engine with per-event rewrites). A doc is
/// projected at [`PopularityScore::ZERO`]; a separate low-frequency signal path
/// updates it on a slow cadence. Precise, live counts stay in `engagement` and are
/// resolved by the caller, never here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PopularityScore(f64);

impl PopularityScore {
    pub const ZERO: Self = Self(0.0);

    /// Clamp to non-negative; a negative popularity is meaningless.
    pub fn new(value: f64) -> Self {
        Self(value.max(0.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_negative_to_zero() {
        assert_eq!(PopularityScore::new(-3.0).value(), 0.0);
    }

    #[test]
    fn preserves_non_negative() {
        assert_eq!(PopularityScore::new(12.5).value(), 12.5);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(PopularityScore::default(), PopularityScore::ZERO);
    }
}
