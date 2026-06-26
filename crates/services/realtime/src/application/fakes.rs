//! In-memory fakes for the four ports, plus a [`Fixture`] composition root, for
//! the application unit tests. They model the semantics that matter — the
//! registry's `user → placements` routing, its fail-open `unavailable` toggle, the
//! node channel's per-node publish record — without any container.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::application::event::DeliverableEvent;
use crate::application::fanout::FanOutHandler;
use crate::application::handshake::HandshakeHandler;
use crate::application::lifecycle::ReapHandler;
use crate::application::port::{
    ConnectionLocation, ConnectionRegistry, EventSource, NodeChannel, TokenVerifier,
};
use crate::domain::{DeviceId, NodeId, Session, UserId};
use crate::error::RealtimeError;

// ── TokenVerifier (auth-context analogue) ─────────────────────────────────────

/// Maps a raw token string to the `(user, device)` it authenticates. An unseeded
/// token is rejected.
#[derive(Default)]
pub struct FakeTokenVerifier {
    valid: Mutex<HashMap<String, (String, String)>>,
}

impl FakeTokenVerifier {
    /// Register a token that verifies to `(user, device)`, with a one-hour expiry
    /// from the verification clock.
    pub fn seed_valid(&self, token: &str, user: &str, device: &str) {
        self.valid
            .lock()
            .unwrap()
            .insert(token.to_owned(), (user.to_owned(), device.to_owned()));
    }
}

#[async_trait]
impl TokenVerifier for FakeTokenVerifier {
    async fn verify(
        &self,
        edge_token: &str,
        now: DateTime<Utc>,
    ) -> Result<Session, RealtimeError> {
        let guard = self.valid.lock().unwrap();
        match guard.get(edge_token) {
            Some((user, device)) => Ok(Session::new(
                UserId::new(user.clone())?,
                DeviceId::new(device.clone())?,
                now + Duration::hours(1),
            )),
            None => Err(RealtimeError::HandshakeRejected {
                reason: "unrecognized edge token".to_owned(),
            }),
        }
    }
}

// ── ConnectionRegistry (Redis routing analogue) ───────────────────────────────

#[derive(Default)]
pub struct InMemoryRegistry {
    placements: Mutex<Vec<ConnectionLocation>>,
    unavailable: AtomicBool,
}

impl InMemoryRegistry {
    pub fn seed(&self, location: ConnectionLocation) {
        self.placements.lock().unwrap().push(location);
    }

    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    fn guard(&self) -> Result<(), RealtimeError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(RealtimeError::RegistryUnavailable);
        }
        Ok(())
    }
}

#[async_trait]
impl ConnectionRegistry for InMemoryRegistry {
    async fn bind(&self, location: &ConnectionLocation) -> Result<(), RealtimeError> {
        self.guard()?;
        self.placements.lock().unwrap().push(location.clone());
        Ok(())
    }

    async fn evict(
        &self,
        user_id: &UserId,
        connection_id: &crate::domain::ConnectionId,
    ) -> Result<(), RealtimeError> {
        self.guard()?;
        self.placements.lock().unwrap().retain(|loc| {
            !(loc.user_id == *user_id && loc.connection_id == *connection_id)
        });
        Ok(())
    }

    async fn resolve(&self, user_id: &UserId) -> Result<Vec<ConnectionLocation>, RealtimeError> {
        self.guard()?;
        Ok(self
            .placements
            .lock()
            .unwrap()
            .iter()
            .filter(|loc| loc.user_id == *user_id)
            .cloned()
            .collect())
    }
}

// ── NodeChannel (Redis Pub/Sub analogue) ──────────────────────────────────────

#[derive(Default)]
pub struct FakeNodeChannel {
    published: Mutex<Vec<(NodeId, DeliverableEvent)>>,
    unavailable: AtomicBool,
}

impl FakeNodeChannel {
    pub fn set_unavailable(&self, down: bool) {
        self.unavailable.store(down, Ordering::SeqCst);
    }

    pub fn publish_count(&self) -> usize {
        self.published.lock().unwrap().len()
    }

    pub fn published_to(&self, node: &str) -> usize {
        self.published
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| n.as_str() == node)
            .count()
    }
}

#[async_trait]
impl NodeChannel for FakeNodeChannel {
    async fn publish(
        &self,
        node_id: &NodeId,
        event: &DeliverableEvent,
    ) -> Result<(), RealtimeError> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(RealtimeError::NodeChannelUnavailable);
        }
        self.published
            .lock()
            .unwrap()
            .push((node_id.clone(), event.clone()));
        Ok(())
    }
}

// ── EventSource (Kafka feed analogue) ─────────────────────────────────────────

#[derive(Default)]
pub struct FakeEventSource {
    queue: Mutex<VecDeque<DeliverableEvent>>,
}

impl FakeEventSource {
    pub fn push(&self, event: DeliverableEvent) {
        self.queue.lock().unwrap().push_back(event);
    }

    pub fn is_drained(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }
}

#[async_trait]
impl EventSource for FakeEventSource {
    async fn next_event(&self) -> Result<Option<DeliverableEvent>, RealtimeError> {
        Ok(self.queue.lock().unwrap().pop_front())
    }
}

// ── Composition root ──────────────────────────────────────────────────────────

/// Wires the fakes into the application handlers, the way the real composition
/// roots (Phase 5) wire the live adapters.
pub struct Fixture {
    pub verifier: Arc<FakeTokenVerifier>,
    pub registry: Arc<InMemoryRegistry>,
    pub channel: Arc<FakeNodeChannel>,
    pub source: Arc<FakeEventSource>,
    pub node_id: NodeId,
    pub subscription_cap: usize,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            verifier: Arc::new(FakeTokenVerifier::default()),
            registry: Arc::new(InMemoryRegistry::default()),
            channel: Arc::new(FakeNodeChannel::default()),
            source: Arc::new(FakeEventSource::default()),
            node_id: NodeId::new("node-test").unwrap(),
            subscription_cap: 64,
        }
    }

    /// A fixed wall clock for deterministic tests.
    pub fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 26, 12, 0, 0).unwrap()
    }

    pub fn now(&self) -> DateTime<Utc> {
        Self::fixed_now()
    }

    pub fn handshake(&self) -> HandshakeHandler {
        HandshakeHandler::new(
            self.verifier.clone(),
            self.registry.clone(),
            self.node_id.clone(),
            self.subscription_cap,
        )
    }

    pub fn fanout(&self) -> FanOutHandler {
        FanOutHandler::new(self.registry.clone(), self.channel.clone())
    }

    pub fn reap(&self) -> ReapHandler {
        ReapHandler::new(self.registry.clone())
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}
