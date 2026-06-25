use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{Confidence, PolicyCategory};
use crate::error::ModerationError;

/// A single integrity signal contributing to a [`Case`](crate::domain::aggregate::Case):
/// a classifier verdict or a user-report-derived observation. Signals are inputs
/// to a decision, never the decision itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Origin, e.g. `"classifier:image-v3"` or `"report"`.
    source: String,
    category: PolicyCategory,
    confidence: Confidence,
    observed_at: DateTime<Utc>,
}

impl Signal {
    pub fn new(
        source: impl Into<String>,
        category: PolicyCategory,
        confidence: Confidence,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, ModerationError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(ModerationError::SignalRejected {
                reason: "signal source must not be empty".into(),
            });
        }
        Ok(Self { source, category, confidence, observed_at })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn category(&self) -> PolicyCategory {
        self.category
    }

    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    #[test]
    fn rejects_empty_source() {
        let err = Signal::new("", PolicyCategory::Spam, Confidence::new(0.9).unwrap(), t0()).unwrap_err();
        assert!(matches!(err, ModerationError::SignalRejected { .. }));
    }

    #[test]
    fn carries_fields() {
        let s = Signal::new(
            "classifier:text-v2",
            PolicyCategory::Harassment,
            Confidence::new(0.8).unwrap(),
            t0(),
        )
        .unwrap();
        assert_eq!(s.source(), "classifier:text-v2");
        assert_eq!(s.category(), PolicyCategory::Harassment);
        assert!(s.confidence().at_least(0.8));
    }
}
