use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CounterError;

/// The width of a tumbling pre-aggregation window, in milliseconds. This is the
/// knob that sets the N→1 collapse ratio: all observations for one `(entity,
/// metric)` within a window fold into a single delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
    millis: u64,
}

impl WindowSize {
    /// Build a window size. Zero is rejected — a zero-width window cannot
    /// aggregate (surfaced as `CTR-9001 DomainViolation`).
    pub fn from_millis(millis: u64) -> Result<Self, CounterError> {
        if millis == 0 {
            return Err(CounterError::DomainViolation {
                field: "window_ms".to_owned(),
                message: "window size must be non-zero".to_owned(),
            });
        }
        Ok(Self { millis })
    }

    pub fn millis(&self) -> u64 {
        self.millis
    }
}

/// The identity of a tumbling window: `floor(event_millis / window_ms)`.
///
/// This is the linchpin of idempotent durability. The durable flush is keyed by
/// `(entity, metric, window_id)`, so a worker crash and Kafka redelivery
/// re-aggregate the *same* `WindowId` and re-apply the *same* delta — an
/// idempotent UPSERT, never a double-add. It is derived purely from **event
/// time** (injected), never from a wall clock, so the same event always lands in
/// the same window regardless of when it is processed. (Counter's analogue of
/// search's monotonic `DocVersion`.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(u64);

impl WindowId {
    /// The window an event falls into, given the window size. A pre-epoch event
    /// time floors to window `0`.
    pub fn for_event(size: WindowSize, occurred_at: DateTime<Utc>) -> Self {
        let millis = occurred_at.timestamp_millis();
        let millis = if millis < 0 { 0 } else { millis as u64 };
        Self(millis / size.millis())
    }

    /// Reconstruct from a stored index (e.g. when replaying a durable row).
    pub fn from_index(index: u64) -> Self {
        Self(index)
    }

    pub fn index(&self) -> u64 {
        self.0
    }

    /// Inclusive start of this window, in epoch milliseconds, under `size`.
    pub fn start_millis(&self, size: WindowSize) -> u64 {
        self.0 * size.millis()
    }

    /// Exclusive end of this window, in epoch milliseconds, under `size`.
    pub fn end_millis(&self, size: WindowSize) -> u64 {
        self.start_millis(size) + size.millis()
    }

    /// Whether this window has fully elapsed relative to a watermark — i.e. no
    /// further events can land in it, so it is safe to flush. The watermark is the
    /// event-time frontier (injected), not a wall clock.
    pub fn is_closed_at(&self, size: WindowSize, watermark: DateTime<Utc>) -> bool {
        let wm = watermark.timestamp_millis();
        let wm = if wm < 0 { 0 } else { wm as u64 };
        self.end_millis(size) <= wm
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    #[test]
    fn zero_window_rejected() {
        assert!(WindowSize::from_millis(0).is_err());
    }

    #[test]
    fn events_in_same_window_share_id() {
        let size = WindowSize::from_millis(5_000).unwrap();
        let a = WindowId::for_event(size, at(10_000));
        let b = WindowId::for_event(size, at(14_999));
        assert_eq!(a, b);
        assert_eq!(a.index(), 2);
    }

    #[test]
    fn window_boundary_splits() {
        let size = WindowSize::from_millis(5_000).unwrap();
        let a = WindowId::for_event(size, at(14_999));
        let b = WindowId::for_event(size, at(15_000));
        assert_ne!(a, b);
        assert_eq!(b.index(), 3);
    }

    #[test]
    fn start_and_end_bracket_the_window() {
        let size = WindowSize::from_millis(5_000).unwrap();
        let w = WindowId::for_event(size, at(12_345));
        assert_eq!(w.start_millis(size), 10_000);
        assert_eq!(w.end_millis(size), 15_000);
    }

    #[test]
    fn closes_only_once_fully_elapsed() {
        let size = WindowSize::from_millis(5_000).unwrap();
        let w = WindowId::from_index(2); // [10_000, 15_000)
        assert!(!w.is_closed_at(size, at(14_999)));
        assert!(w.is_closed_at(size, at(15_000)));
        assert!(w.is_closed_at(size, at(20_000)));
    }

    #[test]
    fn pre_epoch_floors_to_window_zero() {
        let size = WindowSize::from_millis(5_000).unwrap();
        assert_eq!(WindowId::for_event(size, at(-1)).index(), 0);
    }
}
