use chrono::Duration;

/// Time policy for session and token lifetimes, resolved from configuration at
/// the composition root (Phase 5) and injected into the handlers.
///
/// Relationships the handlers rely on: `access_ttl ≤ session_ttl ≤ absolute_ttl`
/// (the domain clamps regardless), and the blacklist TTL equals `access_ttl` —
/// long enough to outlive any access token a session could have minted.
#[derive(Debug, Clone)]
pub struct SessionPolicy {
    /// Edge access-token lifetime (short — minutes).
    pub access_ttl: Duration,
    /// Sliding session window, extended on each refresh.
    pub session_ttl: Duration,
    /// Hard cap from issue time; a session can never live past this.
    pub absolute_ttl: Duration,
    /// Refresh-token lifetime (long — days).
    pub refresh_ttl: Duration,
}

impl SessionPolicy {
    pub fn new(
        access_ttl: Duration,
        session_ttl: Duration,
        absolute_ttl: Duration,
        refresh_ttl: Duration,
    ) -> Self {
        Self { access_ttl, session_ttl, absolute_ttl, refresh_ttl }
    }
}

#[cfg(test)]
impl SessionPolicy {
    /// A representative production-shaped policy for tests:
    /// 10-minute access, 30-minute sliding session, 8-hour cap, 7-day refresh.
    pub fn test_default() -> Self {
        Self::new(
            Duration::minutes(10),
            Duration::minutes(30),
            Duration::hours(8),
            Duration::days(7),
        )
    }
}
