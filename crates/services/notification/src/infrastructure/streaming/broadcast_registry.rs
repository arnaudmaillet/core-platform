use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::application::port::{NotificationPayload, StreamRegistry};
use crate::domain::value_object::ProfileId;

/// How often the background reaper sweeps senders that have lost all receivers.
/// Memory reclaim latency only — not behaviourally significant — so a constant is
/// sufficient; lower it if churn is extreme.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

/// In-process broadcast registry for gRPC server-streaming connections.
///
/// Each online profile maps to a single `broadcast::Sender`. Multiple gRPC
/// stream sessions for the same profile share the same channel — this handles
/// the case where a user has multiple device connections simultaneously.
///
/// # Lifecycle / memory
///
/// A sender outlives its receivers (dropping the last `Receiver` does not remove
/// the map entry). Left unmanaged the map would grow without bound — one entry
/// for every profile that has *ever* connected. Two mechanisms reclaim entries:
/// - `broadcast` opportunistically drops a sender the moment a send finds zero
///   receivers; and
/// - [`run_reaper`] periodically sweeps any remaining zero-receiver senders
///   (covering profiles that disconnected and never received another event).
///
/// # Backpressure
///
/// If a receiver falls behind by `buffer_size` messages, the `BroadcastStream`
/// wrapper in the gRPC handler surfaces `RecvError::Lagged(n)`, which the handler
/// turns into a stream termination; the client reconnects and re-polls
/// `ListNotifications` to close the gap.
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

    /// Removes every sender that currently has zero receivers and returns how many
    /// were reaped.
    ///
    /// Safe against a concurrent [`subscribe`](StreamRegistry::subscribe): that
    /// method creates its receiver while holding the DashMap shard lock, so a live
    /// sender is never observed at zero receivers in the window between insert and
    /// the first `subscribe()`. A reaped-then-resubscribed profile simply
    /// re-creates its sender, and broadcasts to a zero-receiver sender are no-ops,
    /// so nothing is lost.
    pub fn reap(&self) -> usize {
        let before = self.senders.len();
        self.senders.retain(|_, tx| tx.receiver_count() > 0);
        before.saturating_sub(self.senders.len())
    }

    /// Runs the periodic reaper forever. Spawn once at startup with an
    /// `Arc<BroadcastRegistry>`; without it the sender map grows unbounded.
    pub async fn run_reaper(self: Arc<Self>) {
        let mut interval = tokio::time::interval(REAP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            let reaped = self.reap();
            if reaped > 0 {
                tracing::debug!(
                    reaped,
                    live = self.senders.len(),
                    "broadcast registry reaped idle senders"
                );
            }
        }
    }
}

impl StreamRegistry for BroadcastRegistry {
    fn subscribe(
        &self,
        profile_id: &ProfileId,
    ) -> broadcast::Receiver<Arc<NotificationPayload>> {
        let uuid = profile_id.as_uuid();

        // Atomic get-or-create. Subscribing while still holding the `entry` shard
        // guard closes the TOCTOU window in which two concurrent first-connections
        // for the same profile would each create a channel and the second `insert`
        // would overwrite (and orphan) the first device's receiver.
        let sender = self
            .senders
            .entry(uuid)
            .or_insert_with(|| broadcast::channel(self.buffer_size).0);

        sender.subscribe()
    }

    fn broadcast(&self, profile_id: &ProfileId, payload: Arc<NotificationPayload>) {
        let uuid = profile_id.as_uuid();

        // Hold the read guard only for the send; it is dropped at the end of this
        // statement, before any removal, so the removal cannot self-deadlock.
        let send_result = self.senders.get(&uuid).map(|sender| sender.send(payload));

        match send_result {
            Some(Ok(receivers)) => {
                tracing::trace!(
                    profile_id = %profile_id,
                    receivers,
                    "notification broadcast delivered"
                );
            }
            Some(Err(_)) => {
                // No active receivers. Reclaim the sender now instead of waiting for
                // the periodic reaper. `remove_if` re-checks the receiver count under
                // the shard lock, so a subscriber that attached between the failed
                // send and here is not torn down. The write to ScyllaDB already
                // provides durable delivery, so dropping the broadcast is safe.
                self.senders.remove_if(&uuid, |_, tx| tx.receiver_count() == 0);
                tracing::trace!(
                    profile_id = %profile_id,
                    "broadcast skipped: no active stream receivers"
                );
            }
            None => {}
        }
    }
}
