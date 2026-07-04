use chrono::{DateTime, Duration, Utc};

use crate::domain::value_object::{EventCategory, SubjectPseudonym};
use crate::error::AuditError;

/// How long records of a given category must be retained before they may expire.
/// The retention *floor* is a minimum, not a maximum — a record may never be
/// expired or deleted before it, and a legal hold can extend it indefinitely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub category: EventCategory,
    pub retention: Duration,
}

impl RetentionPolicy {
    pub fn new(category: EventCategory, retention: Duration) -> Self {
        Self { category, retention }
    }

    /// The earliest instant a record first chained at `recorded_at` may expire.
    pub fn floor(&self, recorded_at: DateTime<Utc>) -> DateTime<Utc> {
        recorded_at + self.retention
    }

    /// Whether the retention floor has passed as of `now`.
    pub fn is_past_floor(&self, recorded_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        now >= self.floor(recorded_at)
    }
}

/// A legal hold suspends the retention/erasure lifecycle for a subject — lawful
/// retention (GDPR Art. 17(3)) that overrides an erasure request. A hold is
/// active between `placed_at` and `released_at` (open-ended until released).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalHold {
    pub id: String,
    pub subject: SubjectPseudonym,
    pub placed_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
}

impl LegalHold {
    pub fn placed(
        id: impl Into<String>,
        subject: SubjectPseudonym,
        placed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            subject,
            placed_at,
            released_at: None,
        }
    }

    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        now >= self.placed_at && self.released_at.is_none_or(|released| now < released)
    }

    pub fn covers(&self, subject: &SubjectPseudonym, now: DateTime<Utc>) -> bool {
        self.is_active(now) && &self.subject == subject
    }
}

/// Authorize a crypto-shred (erasure) of `subject` against the active holds. Any
/// active hold covering the subject blocks it — lawful retention wins
/// (`AUD-5002`). Selective field-level shred of unheld data is the caller's
/// concern; this gate is the all-or-nothing subject check.
pub fn authorize_erasure(
    subject: &SubjectPseudonym,
    holds: &[LegalHold],
    now: DateTime<Utc>,
) -> Result<(), AuditError> {
    if holds.iter().any(|h| h.covers(subject, now)) {
        return Err(AuditError::ShredBlockedByLegalHold {
            subject: subject.to_string(),
        });
    }
    Ok(())
}

/// Authorize expiry of a record. It must be past its retention floor
/// (`AUD-6001`) and not under an active hold (`AUD-6002`). Holds are checked
/// first: a held record is never expired regardless of age.
pub fn authorize_expiry(
    subject: Option<&SubjectPseudonym>,
    recorded_at: DateTime<Utc>,
    policy: &RetentionPolicy,
    holds: &[LegalHold],
    now: DateTime<Utc>,
) -> Result<(), AuditError> {
    if let Some(subject) = subject
        && holds.iter().any(|h| h.covers(subject, now))
    {
        return Err(AuditError::LegalHoldActive);
    }
    if !policy.is_past_floor(recorded_at, now) {
        return Err(AuditError::RetentionFloorViolation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use error::AppError;

    use super::*;

    fn at(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 26, h, 0, 0).unwrap()
    }

    fn day(d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, d, 12, 0, 0).unwrap()
    }

    fn subject(s: &str) -> SubjectPseudonym {
        SubjectPseudonym::new(s).unwrap()
    }

    #[test]
    fn floor_is_recorded_at_plus_retention() {
        let p = RetentionPolicy::new(EventCategory::Moderation, Duration::days(7));
        assert_eq!(p.floor(day(1)), day(8));
        assert!(!p.is_past_floor(day(1), day(7)));
        assert!(p.is_past_floor(day(1), day(8)));
    }

    #[test]
    fn active_hold_blocks_erasure() {
        let holds = vec![LegalHold::placed("lh-1", subject("7f3a"), at(9))];
        let err = authorize_erasure(&subject("7f3a"), &holds, at(10)).unwrap_err();
        assert_eq!(err.error_code(), "AUD-5002");
        assert!(!err.is_retryable());
    }

    #[test]
    fn erasure_allowed_for_unheld_subject() {
        let holds = vec![LegalHold::placed("lh-1", subject("other"), at(9))];
        assert!(authorize_erasure(&subject("7f3a"), &holds, at(10)).is_ok());
    }

    #[test]
    fn released_hold_no_longer_blocks() {
        let mut hold = LegalHold::placed("lh-1", subject("7f3a"), at(9));
        hold.released_at = Some(at(11));
        assert!(hold.covers(&subject("7f3a"), at(10)));
        assert!(!hold.covers(&subject("7f3a"), at(11)));
    }

    #[test]
    fn expiry_blocked_before_floor() {
        let p = RetentionPolicy::new(EventCategory::Moderation, Duration::days(7));
        let err = authorize_expiry(None, day(1), &p, &[], day(5)).unwrap_err();
        assert_eq!(err.error_code(), "AUD-6001");
    }

    #[test]
    fn expiry_blocked_by_active_hold_even_past_floor() {
        let p = RetentionPolicy::new(EventCategory::Moderation, Duration::days(7));
        let holds = vec![LegalHold::placed("lh-1", subject("7f3a"), day(1))];
        let err = authorize_expiry(Some(&subject("7f3a")), day(1), &p, &holds, day(30)).unwrap_err();
        assert_eq!(err.error_code(), "AUD-6002");
    }

    #[test]
    fn expiry_allowed_past_floor_without_hold() {
        let p = RetentionPolicy::new(EventCategory::Moderation, Duration::days(7));
        assert!(authorize_expiry(Some(&subject("7f3a")), day(1), &p, &[], day(30)).is_ok());
    }
}
