use std::collections::HashMap;

use crate::domain::value_object::{ChannelRef, SequenceState, StreamSeq};
use crate::error::RealtimeError;

/// The bounded set of channels a connection is subscribed to, each carrying its
/// own [`SequenceState`] (the per-channel sequence + ack watermark).
///
/// This type owns *membership and sequencing*, not authorization — the caller
/// ([`super::connection::Connection`]) authorizes a channel against the session
/// before adding it here. Keeping authz out of the set keeps this purely about
/// "what is subscribed and where is its sequence".
#[derive(Debug, Clone)]
pub struct SubscriptionSet {
    channels: HashMap<ChannelRef, SequenceState>,
    cap: usize,
}

impl SubscriptionSet {
    /// Create an empty set bounded at `cap` channels. The cap is the per-connection
    /// subscription limit that protects node memory from an unbounded fan-in.
    pub fn new(cap: usize) -> Self {
        Self {
            channels: HashMap::new(),
            cap,
        }
    }

    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    pub fn is_subscribed(&self, channel: &ChannelRef) -> bool {
        self.channels.contains_key(channel)
    }

    /// The channels currently subscribed — used by the gateway to clean its
    /// broadcast index on teardown.
    pub fn channels(&self) -> Vec<ChannelRef> {
        self.channels.keys().cloned().collect()
    }

    /// Subscribe to `channel`. Idempotent: re-subscribing to an existing channel
    /// is a no-op that preserves its sequence state (so a duplicate Subscribe
    /// never resets a client's ack progress). Returns `true` if newly added.
    ///
    /// Exceeding `cap` is `RTM-3002 SubscriptionLimitExceeded`.
    pub fn subscribe(&mut self, channel: ChannelRef) -> Result<bool, RealtimeError> {
        if self.channels.contains_key(&channel) {
            return Ok(false);
        }
        if self.channels.len() >= self.cap {
            return Err(RealtimeError::SubscriptionLimitExceeded);
        }
        self.channels.insert(channel, SequenceState::new());
        Ok(true)
    }

    /// Unsubscribe from `channel`. Unsubscribing from a channel that is not
    /// subscribed is `RTM-3003 NotSubscribed`.
    pub fn unsubscribe(&mut self, channel: &ChannelRef) -> Result<(), RealtimeError> {
        self.channels
            .remove(channel)
            .map(|_| ())
            .ok_or_else(|| RealtimeError::NotSubscribed {
                channel: channel.to_string(),
            })
    }

    /// Assign the next stream sequence for an event about to be delivered on
    /// `channel`. `RTM-3003 NotSubscribed` if the connection is not subscribed.
    pub fn issue_seq(&mut self, channel: &ChannelRef) -> Result<StreamSeq, RealtimeError> {
        self.channels
            .get_mut(channel)
            .map(SequenceState::issue)
            .ok_or_else(|| RealtimeError::NotSubscribed {
                channel: channel.to_string(),
            })
    }

    /// Record a client ack up to `seq` on `channel`. `RTM-3003 NotSubscribed` if
    /// not subscribed; `RTM-2004 SequenceViolation` if the ack exceeds what was
    /// issued (propagated from [`SequenceState::ack`]).
    pub fn ack(&mut self, channel: &ChannelRef, seq: StreamSeq) -> Result<(), RealtimeError> {
        self.channels
            .get_mut(channel)
            .ok_or_else(|| RealtimeError::NotSubscribed {
                channel: channel.to_string(),
            })?
            .ack(seq)
    }
}

#[cfg(test)]
mod tests {
    use error::AppError;

    use super::*;
    use crate::domain::value_object::{ChannelClass, ChannelKey, SequenceState};

    fn channel(class: ChannelClass, key: &str) -> ChannelRef {
        ChannelRef::new(class, ChannelKey::new(key).unwrap())
    }

    #[test]
    fn subscribe_is_idempotent_and_preserves_sequence() {
        let mut set = SubscriptionSet::new(8);
        let dm = channel(ChannelClass::Dm, "alice");
        assert!(set.subscribe(dm.clone()).unwrap()); // newly added
        let s1 = set.issue_seq(&dm).unwrap();
        assert_eq!(s1.get(), 1);
        // Re-subscribe must NOT reset the sequence back to 1.
        assert!(!set.subscribe(dm.clone()).unwrap());
        assert_eq!(set.issue_seq(&dm).unwrap().get(), 2);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn enforces_the_cap() {
        let mut set = SubscriptionSet::new(1);
        set.subscribe(channel(ChannelClass::Dm, "alice")).unwrap();
        let err = set
            .subscribe(channel(ChannelClass::Counter, "post-1"))
            .unwrap_err();
        assert_eq!(err.error_code(), "RTM-3002");
    }

    #[test]
    fn unsubscribe_unknown_channel_is_rejected() {
        let mut set = SubscriptionSet::new(8);
        let err = set
            .unsubscribe(&channel(ChannelClass::Dm, "alice"))
            .unwrap_err();
        assert_eq!(err.error_code(), "RTM-3003");
    }

    #[test]
    fn issue_and_ack_on_unsubscribed_channel_are_rejected() {
        let mut set = SubscriptionSet::new(8);
        let dm = channel(ChannelClass::Dm, "alice");
        assert_eq!(set.issue_seq(&dm).unwrap_err().error_code(), "RTM-3003");
        // A valid StreamSeq value (the NotSubscribed check fires before sequencing).
        let seq = SequenceState::new().issue();
        assert_eq!(set.ack(&dm, seq).unwrap_err().error_code(), "RTM-3003");
    }
}
