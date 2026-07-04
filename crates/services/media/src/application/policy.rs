use std::time::Duration as StdDuration;

use chrono::Duration;

/// Tunable policy the media handlers run under. Injected at the composition root
/// (`MediaConfig::from_env`, Phase 5); the domain ships sane defaults so behaviour
/// is well-defined from day one.
#[derive(Debug, Clone)]
pub struct MediaPolicy {
    /// How long an issued pre-signed upload ticket stays valid.
    pub upload_ticket_ttl: Duration,
    /// Lifetime of a minted signed (private) delivery URL.
    pub signed_url_ttl: Duration,
    /// **Content-hash dedup gate (fork B).** When false (the default), every upload
    /// gets a fresh asset + ticket; when true, an incoming upload whose declared
    /// SHA-256 matches an existing READY asset short-circuits to it. Off until the
    /// refcount-aware purge path has live integration coverage.
    pub dedup_enabled: bool,
    /// Hard timeout for the pre-publish moderation Screen call — a slow gate must
    /// not wedge the pipeline (fail-closed on elapse for CSAM-class).
    pub screen_timeout: StdDuration,
}

impl MediaPolicy {
    /// Production defaults: 15-minute upload window, 5-minute signed URLs, dedup
    /// OFF, 200 ms screen timeout.
    pub fn standard() -> Self {
        Self {
            upload_ticket_ttl: Duration::minutes(15),
            signed_url_ttl: Duration::minutes(5),
            dedup_enabled: false,
            screen_timeout: StdDuration::from_millis(200),
        }
    }

    /// Deterministic defaults for unit tests.
    #[cfg(test)]
    pub fn test_default() -> Self {
        Self::standard()
    }
}
