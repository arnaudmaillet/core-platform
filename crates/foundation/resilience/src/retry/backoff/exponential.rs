use std::time::Duration;

use rand::Rng;

use super::strategy::BackoffStrategy;

/// Controls how randomness is added to the computed exponential delay.
///
/// Full jitter is recommended for large fleets — it spreads retries uniformly
/// across the window, eliminating thundering-herd spikes after an outage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum JitterKind {
    /// Pure exponential: `base_ms * 2^(attempt-1)`, capped at `max_ms`. No randomness.
    None,
    /// `rand(0, cap)` — maximum spread; preferred for thundering-herd mitigation.
    /// Default for large fleets.
    #[default]
    Full,
    /// `cap/2 + rand(0, cap/2)` — guarantees at least half the cap as minimum wait.
    Equal,
}

/// Exponential backoff with optional jitter.
///
/// Delay formula (before jitter): `min(base_ms * 2^(attempt-1), max_ms)`.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Base delay in milliseconds (delay for the first retry before jitter).
    pub base_ms: u64,
    /// Hard cap on the computed delay in milliseconds.
    pub max_ms: u64,
    pub jitter: JitterKind,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            base_ms: 50,
            max_ms: 10_000,
            jitter: JitterKind::Full,
        }
    }
}

impl ExponentialBackoff {
    pub fn new(base_ms: u64, max_ms: u64, jitter: JitterKind) -> Self {
        Self { base_ms, max_ms, jitter }
    }
}

impl BackoffStrategy for ExponentialBackoff {
    fn next_delay(&self, attempt: u32) -> Duration {
        // Clamp the exponent to 30 to prevent u64 overflow on pathological attempt counts.
        let exp = (attempt.saturating_sub(1)).min(30) as u64;
        let cap = self.base_ms.saturating_mul(1u64 << exp).min(self.max_ms);

        let ms = match self.jitter {
            JitterKind::None => cap,
            JitterKind::Full => rand::rng().random_range(0..=cap),
            JitterKind::Equal => cap / 2 + rand::rng().random_range(0..=(cap / 2)),
        };

        Duration::from_millis(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_is_bounded_by_max() {
        let backoff = ExponentialBackoff::new(100, 1_000, JitterKind::None);
        for attempt in 1..=20 {
            assert!(backoff.next_delay(attempt) <= Duration::from_millis(1_000));
        }
    }

    #[test]
    fn no_overflow_on_large_attempt() {
        let backoff = ExponentialBackoff::new(50, 10_000, JitterKind::None);
        // Must not panic
        let _ = backoff.next_delay(u32::MAX);
    }
}
