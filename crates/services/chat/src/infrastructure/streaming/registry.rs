use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::domain::value_object::ConversationId;
use crate::infrastructure::routing::PlaneEvent;

/// How often the reaper sweeps senders that have lost all receivers.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

/// In-process fan-out hub for one plane, keyed by conversation.
///
/// Each conversation with ≥1 locally-connected stream maps to a single
/// `broadcast::Sender`. The pod's [`PlaneSubscriber`] receive loop calls
/// [`broadcast`](Self::broadcast) with events arriving from Redis; every local
/// gRPC stream for that conversation holds a `broadcast::Receiver`. So Redis
/// delivers each event once per pod, and the pod re-fans it to its many local
/// streams in-process — the key scaling property of the design.
///
/// Two instances exist per pod: one for the Member Plane and one for the
/// Audience Plane, keeping their fan-out (and backpressure) fully independent.
pub struct ConversationBroadcastRegistry {
    senders:     DashMap<Uuid, broadcast::Sender<Arc<PlaneEvent>>>,
    buffer_size: usize,
}

impl ConversationBroadcastRegistry {
    pub fn new(buffer_size: usize) -> Self {
        Self { senders: DashMap::new(), buffer_size }
    }

    /// Subscribes a new local stream to a conversation, creating the sender on
    /// first interest. Subscribing while holding the shard guard closes the
    /// TOCTOU window where two concurrent first-subscribers would each create a
    /// channel and orphan one.
    pub fn subscribe(&self, conversation_id: &ConversationId) -> broadcast::Receiver<Arc<PlaneEvent>> {
        let sender = self
            .senders
            .entry(conversation_id.as_uuid())
            .or_insert_with(|| broadcast::channel(self.buffer_size).0);
        sender.subscribe()
    }

    /// Fans an event to all local streams subscribed to the conversation. A send
    /// with no receivers reclaims the sender immediately (re-checked under the
    /// shard lock so a concurrent subscriber is not torn down).
    pub fn broadcast(&self, conversation_id: &ConversationId, payload: Arc<PlaneEvent>) {
        let key = conversation_id.as_uuid();
        let result = self.senders.get(&key).map(|tx| tx.send(payload));
        match result {
            Some(Ok(_)) => {}
            Some(Err(_)) => {
                self.senders.remove_if(&key, |_, tx| tx.receiver_count() == 0);
            }
            None => {}
        }
    }

    /// Drops the conversation's sender, terminating every local stream attached
    /// to it (receivers observe `Closed`). Used to tear down the Audience Plane
    /// across all pods when a conversation is unpublished.
    pub fn close(&self, conversation_id: &ConversationId) {
        self.senders.remove(&conversation_id.as_uuid());
    }

    /// Removes every sender with zero receivers; returns how many were reaped.
    pub fn reap(&self) -> usize {
        let before = self.senders.len();
        self.senders.retain(|_, tx| tx.receiver_count() > 0);
        before.saturating_sub(self.senders.len())
    }

    /// Runs the periodic reaper forever. Spawn once per registry at startup.
    pub async fn run_reaper(self: Arc<Self>) {
        let mut interval = tokio::time::interval(REAP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let reaped = self.reap();
            if reaped > 0 {
                tracing::debug!(reaped, live = self.senders.len(), "conversation registry reaped");
            }
        }
    }
}
