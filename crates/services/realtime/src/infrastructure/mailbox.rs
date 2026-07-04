use std::collections::VecDeque;

/// What happened to a frame on enqueue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// The frame was queued normally.
    Accepted,
    /// The queue was full; the oldest pending frame was dropped to make room.
    /// This is the **shed** that protects node memory from a slow consumer — a
    /// dropped fire-and-forget frame is superseded by the next; a dropped
    /// at-least-once frame is re-synced from the SoR on reconnect.
    SheddedOldest,
}

/// A bounded, per-connection outbound queue with drop-oldest shedding — the
/// pure core of the C10M backpressure rule: one slow client must never be able to
/// balloon a gateway node's memory.
///
/// This is a synchronous data structure; the gateway's per-socket task (Phase 5)
/// owns one of these behind the connection's writer and drains it onto the
/// WebSocket. Keeping the policy here, free of any socket, makes the shed
/// semantics unit-testable. When `Reject`-style behaviour is wanted (close a
/// wedged consumer rather than drop), the caller watches [`shed_total`] /
/// [`is_full`] and closes with `RTM-5001 SendQueueOverflow`.
///
/// [`shed_total`]: ConnectionMailbox::shed_total
/// [`is_full`]: ConnectionMailbox::is_full
#[derive(Debug)]
pub struct ConnectionMailbox {
    queue: VecDeque<Vec<u8>>,
    capacity: usize,
    shed_total: u64,
}

impl ConnectionMailbox {
    /// Create a mailbox holding at most `capacity` frames. `capacity` must be at
    /// least 1.
    pub fn new(capacity: usize) -> Self {
        debug_assert!(capacity >= 1, "mailbox capacity must be >= 1");
        Self {
            queue: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
            shed_total: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }

    /// The total number of frames shed over this mailbox's life — a per-connection
    /// backpressure signal the gateway exports as a metric.
    pub fn shed_total(&self) -> u64 {
        self.shed_total
    }

    /// Enqueue an already-serialized frame, shedding the oldest if full.
    pub fn enqueue(&mut self, frame: Vec<u8>) -> EnqueueOutcome {
        if self.is_full() {
            self.queue.pop_front();
            self.shed_total += 1;
            self.queue.push_back(frame);
            EnqueueOutcome::SheddedOldest
        } else {
            self.queue.push_back(frame);
            EnqueueOutcome::Accepted
        }
    }

    /// Pop the next frame to write to the socket (FIFO).
    pub fn dequeue(&mut self) -> Option<Vec<u8>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_up_to_capacity_then_sheds_oldest() {
        let mut mb = ConnectionMailbox::new(2);
        assert_eq!(mb.enqueue(vec![1]), EnqueueOutcome::Accepted);
        assert_eq!(mb.enqueue(vec![2]), EnqueueOutcome::Accepted);
        assert!(mb.is_full());
        // Third frame sheds the oldest (1), keeping the bound.
        assert_eq!(mb.enqueue(vec![3]), EnqueueOutcome::SheddedOldest);
        assert_eq!(mb.len(), 2);
        assert_eq!(mb.shed_total(), 1);
        // FIFO over what remains: 2 then 3.
        assert_eq!(mb.dequeue(), Some(vec![2]));
        assert_eq!(mb.dequeue(), Some(vec![3]));
        assert_eq!(mb.dequeue(), None);
    }

    #[test]
    fn drains_in_fifo_order() {
        let mut mb = ConnectionMailbox::new(4);
        for i in 0..3u8 {
            mb.enqueue(vec![i]);
        }
        assert_eq!(mb.dequeue(), Some(vec![0]));
        assert_eq!(mb.dequeue(), Some(vec![1]));
        assert!(!mb.is_empty());
        assert_eq!(mb.dequeue(), Some(vec![2]));
        assert!(mb.is_empty());
    }
}
