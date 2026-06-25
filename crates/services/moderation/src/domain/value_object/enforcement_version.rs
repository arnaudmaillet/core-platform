use std::fmt;

use serde::{Deserialize, Serialize};

/// Monotonic per-subject enforcement epoch.
///
/// Each new [`EnforcementAction`](crate::domain::aggregate::EnforcementAction) on
/// a subject is stamped with the next version. A reversal must observe the current
/// version, so a stale reversal can never race ahead of a newer re-application
/// (the optimistic guard lives in the projection/repository; this type is the
/// value it compares). Backed by `i64` to match the Postgres `bigint` column and
/// the proto `int64` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnforcementVersion(i64);

impl EnforcementVersion {
    /// The version the first enforcement on a subject is stamped with.
    pub const INITIAL: EnforcementVersion = EnforcementVersion(1);

    pub fn from_i64(value: i64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> i64 {
        self.0
    }

    /// The next version. Saturating, so a (practically impossible) overflow
    /// degrades safely rather than wrapping backwards.
    #[must_use]
    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl Default for EnforcementVersion {
    fn default() -> Self {
        Self::INITIAL
    }
}

impl fmt::Display for EnforcementVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_is_strictly_increasing_and_saturates() {
        let v = EnforcementVersion::INITIAL;
        assert_eq!(v.value(), 1);
        assert!(v.next() > v);
        assert_eq!(v.next().value(), 2);
        let max = EnforcementVersion::from_i64(i64::MAX);
        assert_eq!(max.next(), max);
    }
}
