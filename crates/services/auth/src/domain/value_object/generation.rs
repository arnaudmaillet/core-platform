use std::fmt;

use serde::{Deserialize, Serialize};

/// Monotonic per-account revocation epoch.
///
/// An edge token carries the `Generation` it was minted under. A global sign-out
/// bumps the account's current generation (see [`Generation::next`]); any token
/// whose embedded generation is below the account's current value is logically
/// dead and rejected at the edge with a single cache read — no per-request DB
/// lookup. Backed by `i64` to match the Postgres `bigint` column and the proto
/// `int64` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Generation(i64);

impl Generation {
    /// The generation a brand-new account session registry starts at.
    pub const INITIAL: Generation = Generation(0);

    /// Wraps a raw value from a trusted source (DB row / verified claim).
    pub fn from_i64(value: i64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> i64 {
        self.0
    }

    /// The next epoch after a global sign-out. Saturating, so a (practically
    /// impossible) overflow degrades safely instead of wrapping to a live value.
    #[must_use]
    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl Default for Generation {
    fn default() -> Self {
        Self::INITIAL
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_is_strictly_increasing() {
        let g0 = Generation::INITIAL;
        let g1 = g0.next();
        assert!(g1 > g0);
        assert_eq!(g1.value(), 1);
        assert_eq!(g1.next().value(), 2);
    }

    #[test]
    fn next_saturates_instead_of_wrapping() {
        let max = Generation::from_i64(i64::MAX);
        assert_eq!(max.next(), max);
    }
}
