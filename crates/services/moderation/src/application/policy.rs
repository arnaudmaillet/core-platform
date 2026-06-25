//! Application-level policy: the tunables the use-cases inject into the domain
//! engine. The [`PenaltyPolicy`] drives graduated enforcement; the enforcement
//! TTLs decide how long time-boxed actor restrictions last; the screen policy
//! version pins the automated decisions the Screen gate records.

use chrono::{DateTime, Duration, Utc};

use crate::domain::value_object::{ActionType, PenaltyPolicy, PolicyVersion};

/// Resolved moderation policy, supplied at the composition root (pinned to a
/// policy version in production).
#[derive(Debug, Clone)]
pub struct ModerationPolicy {
    /// The graduated-enforcement ladder + decay + weights.
    pub penalty: PenaltyPolicy,
    /// How long a temporary actor restriction (`RestrictActor`) lasts.
    pub restrict_actor_ttl: Duration,
    /// How long a `Suspend` lasts before lapsing.
    pub suspend_ttl: Duration,
    /// The policy version stamped on automated decisions made by the Screen gate.
    pub screen_policy_version: PolicyVersion,
    /// Hard wall-clock bound on the Plane C corpus lookup. If the corpus does not
    /// answer within this window the screen returns `ScreenUnavailable` — the
    /// caller's per-category fail policy then blocks (catastrophic) or admits
    /// (best-effort). This is what stops a slow/stuck corpus from wedging the
    /// `media`/`post` publish path.
    pub screen_timeout: std::time::Duration,
}

impl ModerationPolicy {
    /// Production-shaped defaults.
    pub fn standard() -> Self {
        Self {
            penalty: PenaltyPolicy::standard(),
            restrict_actor_ttl: Duration::days(7),
            suspend_ttl: Duration::days(30),
            screen_policy_version: PolicyVersion::new("screen-corpus-1")
                .expect("literal screen policy version is valid"),
            screen_timeout: std::time::Duration::from_millis(200),
        }
    }

    /// The expiry an enforcement of `action` should carry, applied at `now`.
    /// `RestrictActor`/`Suspend` are time-boxed; `Ban` and content actions are
    /// permanent (`None`).
    pub fn expiry_for(&self, action: ActionType, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match action {
            ActionType::RestrictActor => Some(now + self.restrict_actor_ttl),
            ActionType::Suspend => Some(now + self.suspend_ttl),
            _ => None,
        }
    }
}

#[cfg(test)]
impl ModerationPolicy {
    /// Deterministic policy for tests (mirrors `standard`).
    pub fn test_default() -> Self {
        Self::standard()
    }
}
