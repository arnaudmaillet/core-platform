//! Error severity classification.
//!
//! [`Severity`] is the single, workspace-wide vocabulary every microservice
//! uses to rank how bad an error is. It is intentionally decoupled from any
//! domain: it only answers "how loud should this be?" — which drives alerting
//! (paging), log levels and client-facing ordering. Keeping it here means a
//! comment-service `Critical` and an auth-service `Critical` mean the same
//! thing to the observability stack.

use std::fmt;

use serde::{Deserialize, Serialize};
use tracing::Level;

/// Severity of an [`AppError`](crate::AppError), from most to least urgent.
///
/// The `Ord`/`PartialOrd` derives follow declaration order, so
/// `Severity::Critical < Severity::Info`. Callers that want "at least this
/// urgent" semantics should compare explicitly rather than rely on the numeric
/// ordering direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Service is down or data integrity is at risk. Always pages.
    Critical,
    /// Significant degradation impacting users. Pages.
    High,
    /// Recoverable or partial failure. Default for unclassified errors.
    Medium,
    /// Minor issue, expected under normal operation (e.g. validation).
    Low,
    /// Purely informational, not an operational concern.
    Info,
}

impl Severity {
    /// Returns `true` for severities that should trigger an on-call page.
    ///
    /// Only [`Severity::Critical`] and [`Severity::High`] page; everything else
    /// is expected to be handled asynchronously via dashboards and logs.
    pub fn should_page(&self) -> bool {
        matches!(self, Severity::Critical | Severity::High)
    }

    /// Maps the severity onto the [`tracing::Level`] used when emitting logs.
    ///
    /// This is the canonical translation used by
    /// [`DistributedError::log`](crate::DistributedError::log) so that severity
    /// and log level never drift apart across services.
    pub fn log_level(&self) -> Level {
        match self {
            Severity::Critical | Severity::High => Level::ERROR,
            Severity::Medium => Level::WARN,
            Severity::Low => Level::INFO,
            Severity::Info => Level::DEBUG,
        }
    }

    /// Stable, human-readable label. Matches the `Serialize` representation so
    /// logs and JSON payloads agree.
    pub fn as_label(&self) -> &'static str {
        match self {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
            Severity::Info => "Info",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}
