use serde::{Deserialize, Serialize};

/// The derived liveness of a connection/user.
///
/// In v1 presence is **internal liveness only** (a byproduct of connection state
/// used for reaping and the offline-push handoff to `notification`), not a
/// product-facing contract — see `project_realtime_blueprint`. It is *derived*,
/// never authoritative: it falls out of whether a connection is active and its
/// heartbeat is fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresenceState {
    /// An active connection with a fresh heartbeat.
    Online,
    /// Connected, but the heartbeat is overdue (within the reap grace window) —
    /// likely a radio sleep or a flaky link, not yet reaped.
    Away,
    /// No live connection (closed, or reaped after the heartbeat deadline).
    Offline,
}

impl PresenceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PresenceState::Online => "online",
            PresenceState::Away => "away",
            PresenceState::Offline => "offline",
        }
    }
}
