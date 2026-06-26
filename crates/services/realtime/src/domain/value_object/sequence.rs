use serde::{Deserialize, Serialize};

use crate::error::RealtimeError;

/// A per-`(connection, channel)` monotonic stream sequence number. Starts at 1
/// for the first event delivered on a channel; the client dedupes on it, acks on
/// it (at-least-once channels), and presents it as its resume cursor.
///
/// Sequences are scoped to a *connection*: a reconnect is a fresh connection and
/// restarts at 1. The plane buffers nothing, so cross-connection gap-fill is the
/// client's job against the owning SoR — the sequence is a within-connection
/// ordering/dedup device, not a durable log offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StreamSeq(u64);

impl StreamSeq {
    /// Wrap a raw sequence value — used at the wire boundary to reconstruct the
    /// sequence a client acked. Whether it is in range is validated by
    /// [`SequenceState::ack`], not here.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for StreamSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The sequence + ack state for one subscribed channel on one connection.
///
/// Invariant: `0 <= ack_watermark < next`. `next` is the value the *next*
/// delivered event will carry; `ack_watermark` is the highest sequence the
/// client has acknowledged (always 0 for fire-and-forget channels, which are
/// never acked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceState {
    next: u64,
    ack_watermark: u64,
}

impl Default for SequenceState {
    fn default() -> Self {
        Self::new()
    }
}

impl SequenceState {
    pub fn new() -> Self {
        Self {
            next: 1,
            ack_watermark: 0,
        }
    }

    /// Assign the next sequence to an event about to be delivered, advancing the
    /// counter. Infallible and monotonic.
    pub fn issue(&mut self) -> StreamSeq {
        let seq = self.next;
        self.next += 1;
        StreamSeq(seq)
    }

    /// The highest sequence handed out so far (0 if none issued yet).
    pub fn last_issued(&self) -> u64 {
        self.next - 1
    }

    /// The current ack watermark.
    pub fn ack_watermark(&self) -> u64 {
        self.ack_watermark
    }

    /// Record a client acknowledgement up to `seq`.
    ///
    /// * Acking beyond what was issued is impossible and rejected as
    ///   `RTM-2004 SequenceViolation` (a malformed or hostile client).
    /// * A duplicate / out-of-order ack at or below the current watermark is a
    ///   benign no-op (at-least-once delivery means clients may re-ack).
    /// * Otherwise the watermark advances to `seq`.
    pub fn ack(&mut self, seq: StreamSeq) -> Result<(), RealtimeError> {
        if seq.0 > self.last_issued() {
            return Err(RealtimeError::SequenceViolation {
                reason: format!(
                    "ack {} exceeds last issued {}",
                    seq.0,
                    self.last_issued()
                ),
            });
        }
        if seq.0 > self.ack_watermark {
            self.ack_watermark = seq.0;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;

    #[test]
    fn issues_monotonically_from_one() {
        let mut s = SequenceState::new();
        assert_eq!(s.issue().get(), 1);
        assert_eq!(s.issue().get(), 2);
        assert_eq!(s.issue().get(), 3);
        assert_eq!(s.last_issued(), 3);
    }

    #[test]
    fn ack_advances_watermark() {
        let mut s = SequenceState::new();
        let _ = s.issue();
        let two = s.issue();
        s.ack(two).unwrap();
        assert_eq!(s.ack_watermark(), 2);
    }

    #[test]
    fn duplicate_or_stale_ack_is_a_noop() {
        let mut s = SequenceState::new();
        let one = s.issue();
        let _ = s.issue();
        s.ack(one).unwrap();
        s.ack(one).unwrap(); // re-ack tolerated
        assert_eq!(s.ack_watermark(), 1);
    }

    #[test]
    fn ack_beyond_issued_is_a_sequence_violation() {
        let mut s = SequenceState::new();
        let one = s.issue(); // last_issued == 1
        let _ = one;
        // Forge a sequence the server never handed out.
        let forged = {
            let mut tmp = SequenceState::new();
            tmp.issue();
            tmp.issue();
            tmp.issue() // StreamSeq(3)
        };
        let err = s.ack(forged).unwrap_err();
        assert_eq!(err.error_code(), "RTM-2004");
    }
}
