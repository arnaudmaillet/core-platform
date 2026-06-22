use std::collections::VecDeque;

use uuid::Uuid;

/// In-memory accumulator for events targeting the same `(target, subject, kind)`
/// within a single Kafka poll batch.
///
/// Kept simple by design: no Redis round-trips, no locking beyond the owning
/// `HashMap`. Instantiated fresh for each poll cycle and flushed to ScyllaDB
/// after all events in the batch are processed.
#[derive(Debug, Default)]
pub struct CollapseBuffer {
    /// Distinct sender UUIDs collected in FIFO order.
    /// Capped at `max_sample_senders` — only the first N are stored.
    pub senders:          VecDeque<Uuid>,
    /// Total number of events accumulated (may exceed `senders.len()`).
    pub total_count:      u32,
    /// Timestamp of the first event in this batch window.
    pub first_at_ms:      i64,
    /// Timestamp of the most-recent event in this batch window.
    pub last_at_ms:       i64,
}

impl CollapseBuffer {
    pub fn new(sender_id: Uuid, event_at_ms: i64, max_sample: usize) -> Self {
        let mut senders = VecDeque::with_capacity(max_sample);
        senders.push_back(sender_id);
        Self {
            senders,
            total_count: 1,
            first_at_ms: event_at_ms,
            last_at_ms:  event_at_ms,
        }
    }

    /// Accumulates one more event into the buffer.
    pub fn push(&mut self, sender_id: Uuid, event_at_ms: i64, max_sample: usize) {
        self.total_count += 1;
        self.last_at_ms   = self.last_at_ms.max(event_at_ms);

        if self.senders.len() < max_sample && !self.senders.contains(&sender_id) {
            self.senders.push_back(sender_id);
        }
    }

    pub fn primary_sender(&self) -> Uuid {
        // The most-recently-seen sender is the first element (FIFO = insertion order).
        *self.senders.front().expect("buffer is never empty")
    }

    pub fn sample_sender_ids(&self) -> Vec<Uuid> {
        self.senders.iter().copied().collect()
    }

    pub fn sender_count(&self) -> i32 {
        self.total_count as i32
    }
}
