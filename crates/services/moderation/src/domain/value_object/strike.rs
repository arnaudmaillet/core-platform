use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::PolicyCategory;

/// A single penalty point-bearing mark against an actor, recorded when a decision
/// enforces against them. Points and the decay deadline are **snapshotted** at
/// record time (from the policy then in force) so that a later policy change does
/// not retroactively re-weight history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    category: PolicyCategory,
    points: u32,
    recorded_at: DateTime<Utc>,
    /// When this strike stops counting toward the active total (decay).
    expires_at: DateTime<Utc>,
}

impl Strike {
    pub fn new(
        category: PolicyCategory,
        points: u32,
        recorded_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self { category, points, recorded_at, expires_at }
    }

    pub fn category(&self) -> PolicyCategory {
        self.category
    }

    pub fn points(&self) -> u32 {
        self.points
    }

    pub fn recorded_at(&self) -> DateTime<Utc> {
        self.recorded_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Whether this strike still counts toward the active total at `now`.
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }
}
