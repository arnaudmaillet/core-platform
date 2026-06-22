use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::application::port::{NotificationPayload, StreamRegistry};
use crate::domain::value_object::ProfileId;

/// In-process broadcast registry for gRPC server-streaming connections.
///
/// Each online profile maps to a single `broadcast::Sender`. Multiple gRPC
/// stream sessions for the same profile share the same channel — this handles
/// the case where a user has multiple device connections simultaneously.
///
/// Channel lifecycle:
/// - A sender is created lazily on the first `subscribe` call for a profile.
/// - When a subscriber drops its `Receiver`, the `broadcast::Sender::receiver_count`
///   drops. Senders with zero receivers are NOT reaped proactively here; fred
///   memory overhead is negligible (~64 bytes per entry) and the DashMap
///   remains bounded by the number of concurrent online sessions.
///
/// Backpressure:
/// - If a receiver falls behind by `buffer_size` messages, `send()` returns
///   `SendError::Lagged(n)` on the *receiver* side. The `BroadcastStream` wrapper
///   in the gRPC handler converts this to `RecvError::Lagged(n)`, which the
///   handler translates into a stream termination signal. The client reconnects
///   and re-polls `ListNotifications` to close the gap.
pub struct BroadcastRegistry {
    senders:     DashMap<Uuid, broadcast::Sender<Arc<NotificationPayload>>>,
    buffer_size: usize,
}

impl BroadcastRegistry {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            senders: DashMap::new(),
            buffer_size,
        }
    }
}

impl StreamRegistry for BroadcastRegistry {
    fn subscribe(
        &self,
        profile_id: &ProfileId,
    ) -> broadcast::Receiver<Arc<NotificationPayload>> {
        let uuid = profile_id.as_uuid();

        // Return a new receiver from an existing sender, or create one.
        if let Some(entry) = self.senders.get(&uuid) {
            return entry.subscribe();
        }

        let (tx, rx) = broadcast::channel(self.buffer_size);
        self.senders.insert(uuid, tx);
        rx
    }

    fn broadcast(&self, profile_id: &ProfileId, payload: Arc<NotificationPayload>) {
        let uuid = profile_id.as_uuid();

        if let Some(sender) = self.senders.get(&uuid) {
            match sender.send(payload) {
                Ok(n) => {
                    tracing::trace!(
                        profile_id = %profile_id,
                        receivers  = n,
                        "notification broadcast delivered"
                    );
                }
                Err(_) => {
                    // No active receivers — silently discard. The write to
                    // ScyllaDB already provides durable delivery.
                    tracing::trace!(
                        profile_id = %profile_id,
                        "broadcast skipped: no active stream receivers"
                    );
                }
            }
        }
    }
}
