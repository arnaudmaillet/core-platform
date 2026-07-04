use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A monotonic version stamped on every index document, derived from the source
/// entity's own revision / event time.
///
/// This is the linchpin of out-of-order correctness: writes go to the engine with
/// `version_type=external`, so the engine atomically rejects any upsert whose
/// `DocVersion` is not strictly greater than the stored one. A stale, replayed, or
/// reordered event therefore can never clobber a newer document — no locks, no
/// read-modify-write in the consumer. (The search analogue of `moderation`'s
/// monotonic per-subject enforcement version.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DocVersion(u64);

impl DocVersion {
    /// Wrap an explicit source revision (e.g. a post's update sequence). Prefer
    /// this when the source emits a monotonic counter.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Derive a version from an event timestamp (epoch milliseconds). Used when the
    /// source has no explicit counter — e.g. a moderation visibility flip keyed on
    /// `occurred_at`. A pre-epoch timestamp floors to `0`.
    pub fn from_event_time(occurred_at: DateTime<Utc>) -> Self {
        let millis = occurred_at.timestamp_millis();
        Self(if millis < 0 { 0 } else { millis as u64 })
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    /// Whether this version supersedes `other` — the same predicate the engine's
    /// external-version guard applies.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.0 > other.0
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn explicit_revision_is_preserved() {
        assert_eq!(DocVersion::new(42).value(), 42);
    }

    #[test]
    fn event_time_maps_to_millis() {
        let ts = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        assert_eq!(DocVersion::from_event_time(ts).value(), 1_700_000_000_000);
    }

    #[test]
    fn pre_epoch_floors_to_zero() {
        let ts = Utc.timestamp_opt(-5, 0).unwrap();
        assert_eq!(DocVersion::from_event_time(ts).value(), 0);
    }

    #[test]
    fn newer_than_is_strict() {
        let older = DocVersion::new(1);
        let newer = DocVersion::new(2);
        assert!(newer.is_newer_than(&older));
        assert!(!older.is_newer_than(&newer));
        assert!(!newer.is_newer_than(&newer));
    }
}
