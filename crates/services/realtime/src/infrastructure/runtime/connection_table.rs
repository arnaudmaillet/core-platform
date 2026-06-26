//! The node-local connection table: the in-process half of the routing fabric.
//!
//! The Redis registry answers "*which node* holds this user?"; this table answers
//! "*which sockets on THIS node*?". The node subscriber resolves an inbound
//! `DeliverEnvelope` against it and pushes an `Event` frame into each matching
//! connection's outbound queue — assigning that connection's per-channel sequence
//! as it goes (the sequence state lives in the shared [`Connection`]).

use std::sync::Arc;

use dashmap::DashMap;
use prost::Message;
use realtime_api as pb;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use crate::application::DeliverableEvent;
use crate::domain::{Connection, ConnectionId, DeviceId};
use crate::infrastructure::codec;

/// A live connection's node-local handle: its shared domain state (for sequencing
/// + subscription checks) and the bounded sender feeding its socket writer task.
#[derive(Clone)]
pub struct ConnHandle {
    pub connection_id: ConnectionId,
    pub device_id: DeviceId,
    pub connection: Arc<Mutex<Connection>>,
    /// Bounded outbound queue; a full queue means a slow consumer — `try_send`
    /// drops the frame (the shed) rather than blocking the deliver path.
    pub sender: mpsc::Sender<Vec<u8>>,
}

/// Maps a `user_id` to the handles of its live connections on this node.
#[derive(Default)]
pub struct ConnectionTable {
    by_user: DashMap<String, Vec<ConnHandle>>,
}

impl ConnectionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, user_id: &str, handle: ConnHandle) {
        self.by_user.entry(user_id.to_owned()).or_default().push(handle);
    }

    /// Remove one connection; drops the user's entry entirely when it empties.
    pub fn remove(&self, user_id: &str, connection_id: &ConnectionId) {
        if let Some(mut handles) = self.by_user.get_mut(user_id) {
            handles.retain(|h| &h.connection_id != connection_id);
        }
        self.by_user.remove_if(user_id, |_, v| v.is_empty());
    }

    pub fn connection_count(&self) -> usize {
        self.by_user.iter().map(|e| e.value().len()).sum()
    }

    /// Deliver an event to the recipient's matching connections on this node,
    /// stamping each with that connection's next per-channel sequence. Returns the
    /// number of sockets the frame was queued onto.
    ///
    /// Skips connections not subscribed to the event's channel (a non-error), and
    /// honours an optional device filter. A full outbound queue sheds the frame
    /// for that connection (the slow-consumer protection) without affecting others.
    pub async fn deliver(&self, event: &DeliverableEvent) -> usize {
        let Some(handles) = self
            .by_user
            .get(event.recipient.as_str())
            .map(|e| e.value().clone())
        else {
            return 0;
        };

        let mut delivered = 0;
        for handle in handles {
            if let Some(device) = &event.device_id
                && &handle.device_id != device
            {
                continue;
            }

            let frame = {
                let mut conn = handle.connection.lock().await;
                if !conn.is_subscribed(&event.channel) {
                    continue;
                }
                match conn.issue_seq(&event.channel) {
                    Ok(seq) => codec::event_frame(&event.channel, seq, event).encode_to_vec(),
                    Err(_) => continue,
                }
            };

            // try_send: a full queue is a slow consumer — shed rather than block.
            if handle.sender.try_send(frame).is_ok() {
                delivered += 1;
            }
        }
        delivered
    }

    /// Graceful drain (rollout): mark every connection `Draining` and push a
    /// `RECONNECT` control frame carrying the base backoff. The client adds jitter
    /// on top — mandatory, to spread the reconnect across the fleet instead of
    /// stampeding the auth handshake path. Returns the number of connections
    /// signalled. The owning per-socket tasks then close on their own terms.
    ///
    /// Handles are snapshotted before any `await` so no DashMap shard guard is held
    /// across the per-connection lock (a deadlock/`!Send` hazard otherwise).
    pub async fn broadcast_drain(&self, reconnect_after_ms: u32) -> usize {
        let handles: Vec<ConnHandle> =
            self.by_user.iter().flat_map(|e| e.value().clone()).collect();
        let frame = codec::control_frame(
            pb::ServerControl::Reconnect,
            None,
            "RTM-5003",
            "node draining; reconnect with backoff",
            reconnect_after_ms,
        )
        .encode_to_vec();

        let mut signalled = 0;
        for handle in handles {
            let _ = handle.connection.lock().await.begin_drain();
            if handle.sender.try_send(frame.clone()).is_ok() {
                signalled += 1;
            }
        }
        signalled
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::domain::{
        ChannelClass, ChannelKey, ChannelRef, NodeId, Session, UserId,
    };

    fn deliverable(recipient: &str, device: Option<&str>) -> DeliverableEvent {
        DeliverableEvent {
            recipient: UserId::new(recipient).unwrap(),
            device_id: device.map(|d| DeviceId::new(d).unwrap()),
            channel: ChannelRef::new(ChannelClass::Dm, ChannelKey::new(recipient).unwrap()),
            payload: b"x".to_vec(),
            event_type: "chat.message".to_owned(),
            emitted_at: Utc::now(),
            idempotency_key: "e1".to_owned(),
        }
    }

    fn handle(user: &str, device: &str, conn: &str) -> (ConnHandle, mpsc::Receiver<Vec<u8>>) {
        handle_cap(user, device, conn, 8)
    }

    fn handle_cap(
        user: &str,
        device: &str,
        conn: &str,
        cap: usize,
    ) -> (ConnHandle, mpsc::Receiver<Vec<u8>>) {
        let session = Session::new(
            UserId::new(user).unwrap(),
            DeviceId::new(device).unwrap(),
            Utc::now() + chrono::Duration::hours(1),
        );
        let connection = Connection::open(
            ConnectionId::new(conn).unwrap(),
            NodeId::new("node-1").unwrap(),
            session,
            16,
            Utc::now(),
        );
        let (tx, rx) = mpsc::channel(cap);
        (
            ConnHandle {
                connection_id: ConnectionId::new(conn).unwrap(),
                device_id: DeviceId::new(device).unwrap(),
                connection: Arc::new(Mutex::new(connection)),
                sender: tx,
            },
            rx,
        )
    }

    async fn subscribe(handle: &ConnHandle, user: &str) {
        handle
            .connection
            .lock()
            .await
            .subscribe(ChannelRef::new(
                ChannelClass::Dm,
                ChannelKey::new(user).unwrap(),
            ))
            .unwrap();
    }

    #[tokio::test]
    async fn delivers_only_to_subscribed_connections() {
        let table = ConnectionTable::new();
        let (h, mut rx) = handle("alice", "phone", "c1");
        // Subscribe the connection to dm:alice first.
        h.connection
            .lock()
            .await
            .subscribe(ChannelRef::new(
                ChannelClass::Dm,
                ChannelKey::new("alice").unwrap(),
            ))
            .unwrap();
        table.insert("alice", h);

        assert_eq!(table.deliver(&deliverable("alice", None)).await, 1);
        assert!(rx.try_recv().is_ok()); // a frame was queued

        // An unsubscribed second connection receives nothing.
        let (h2, mut rx2) = handle("alice", "tablet", "c2");
        table.insert("alice", h2);
        assert_eq!(table.deliver(&deliverable("alice", None)).await, 1);
        assert!(rx2.try_recv().is_err());
    }

    #[tokio::test]
    async fn offline_user_delivers_to_nobody() {
        let table = ConnectionTable::new();
        assert_eq!(table.deliver(&deliverable("ghost", None)).await, 0);
    }

    #[tokio::test]
    async fn remove_clears_the_entry() {
        let table = ConnectionTable::new();
        let (h, _rx) = handle("alice", "phone", "c1");
        table.insert("alice", h);
        assert_eq!(table.connection_count(), 1);
        table.remove("alice", &ConnectionId::new("c1").unwrap());
        assert_eq!(table.connection_count(), 0);
    }

    #[tokio::test]
    async fn broadcast_drain_marks_draining_and_sends_reconnect() {
        use prost::Message as _;

        let table = ConnectionTable::new();
        let (h, mut rx) = handle("alice", "phone", "c1");
        let conn = Arc::clone(&h.connection);
        table.insert("alice", h);

        assert_eq!(table.broadcast_drain(500).await, 1);

        // The connection is now Draining …
        assert_eq!(
            conn.lock().await.state(),
            crate::domain::ConnectionState::Draining
        );
        // … and a RECONNECT control frame was queued with the base backoff.
        let frame = rx.try_recv().expect("a reconnect frame was queued");
        match super::pb::ServerFrame::decode(&frame[..]).unwrap().body.unwrap() {
            super::pb::server_frame::Body::Control(c) => {
                assert_eq!(c.control, super::pb::ServerControl::Reconnect as i32);
                assert_eq!(c.reconnect_after_ms, 500);
            }
            _ => panic!("expected a Control frame"),
        }
    }

    #[tokio::test]
    async fn full_queue_sheds_without_blocking_or_affecting_others() {
        let table = ConnectionTable::new();
        // A slow consumer with a 1-slot queue that nobody drains.
        let (slow, _slow_rx) = handle_cap("alice", "phone", "c1", 1);
        subscribe(&slow, "alice").await;
        table.insert("alice", slow);

        // First delivery fills the slot; subsequent ones shed (try_send fails) —
        // deliver still returns promptly and never blocks the hot path.
        assert_eq!(table.deliver(&deliverable("alice", None)).await, 1);
        assert_eq!(table.deliver(&deliverable("alice", None)).await, 0);
        assert_eq!(table.deliver(&deliverable("alice", None)).await, 0);
    }
}
