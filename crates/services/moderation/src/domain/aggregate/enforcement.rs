use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::event::{DomainEvent, EnforcementApplied, EnforcementReversed};
use crate::domain::value_object::{
    ActionType, ActorId, DecisionId, EnforcementId, EnforcementStatus, EnforcementVersion,
    SubjectRef,
};
use crate::error::ModerationError;

/// Parameters to apply a new enforcement action.
#[derive(Debug, Clone)]
pub struct EnforcementParams {
    pub subject: SubjectRef,
    pub action: ActionType,
    /// The decision that authorized this enforcement.
    pub decision_id: DecisionId,
    /// The next monotonic version for this subject (supplied by the application
    /// after reading the current max).
    pub version: EnforcementVersion,
    pub applied_at: DateTime<Utc>,
    /// `None` for permanent actions (e.g. `Ban`); set for time-boxed ones.
    pub expires_at: Option<DateTime<Utc>>,
    pub correlation_id: Uuid,
}

/// The **EnforcementAction** aggregate — the executable, lifecycle-bearing
/// consequence of a [`Decision`](crate::domain::aggregate::Decision).
///
/// # Invariants (enforced here)
/// 1. An enforcement is created `Active` and emits [`EnforcementApplied`].
/// 2. It can be reversed only while `Active`; reversing again is rejected
///    (`EnforcementAlreadyReversed`). Reversal emits [`EnforcementReversed`].
/// 3. It can expire only while `Active` and only at/after `expires_at`.
/// 4. It carries a monotonic per-subject `version`; the reversal event re-publishes
///    that version so a consumer can reject a stale reversal (the cross-aggregate
///    race guard lives in the projection, this is the value it checks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementAction {
    id: EnforcementId,
    subject: SubjectRef,
    action: ActionType,
    status: EnforcementStatus,
    version: EnforcementVersion,
    decision_id: DecisionId,
    applied_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    reversed_at: Option<DateTime<Utc>>,

    #[serde(skip)]
    pending_events: Vec<DomainEvent>,
}

impl EnforcementAction {
    /// Applies a new enforcement. Rejects a non-enforcing action (`NoAction` is a
    /// dismissal — it produces a decision but never an enforcement) and an
    /// `expires_at` that is not after `applied_at`.
    pub fn apply(params: EnforcementParams) -> Result<Self, ModerationError> {
        if !params.action.is_enforced() {
            return Err(ModerationError::DomainViolation {
                field: "enforcement.action".into(),
                message: "no_action does not produce an enforcement".into(),
            });
        }
        if let Some(exp) = params.expires_at
            && exp <= params.applied_at
        {
            return Err(ModerationError::DomainViolation {
                field: "enforcement.expires_at".into(),
                message: "expiry must be after the applied time".into(),
            });
        }

        let id = EnforcementId::new();
        let event = DomainEvent::EnforcementApplied(EnforcementApplied {
            enforcement_id: id,
            subject: params.subject.clone(),
            actor_id: params.subject.actor_id(),
            action: params.action,
            version: params.version,
            applied_at: params.applied_at,
            expires_at: params.expires_at,
            occurred_at: params.applied_at,
            correlation_id: params.correlation_id,
        });

        Ok(Self {
            id,
            subject: params.subject,
            action: params.action,
            status: EnforcementStatus::Active,
            version: params.version,
            decision_id: params.decision_id,
            applied_at: params.applied_at,
            expires_at: params.expires_at,
            reversed_at: None,
            pending_events: vec![event],
        })
    }

    /// Reconstructs from storage (no events emitted).
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: EnforcementId,
        subject: SubjectRef,
        action: ActionType,
        status: EnforcementStatus,
        version: EnforcementVersion,
        decision_id: DecisionId,
        applied_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
        reversed_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            subject,
            action,
            status,
            version,
            decision_id,
            applied_at,
            expires_at,
            reversed_at,
            pending_events: Vec::new(),
        }
    }

    // ─── Queries ─────────────────────────────────────────────────────────────

    pub fn id(&self) -> EnforcementId {
        self.id
    }

    pub fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    pub fn actor_id(&self) -> ActorId {
        self.subject.actor_id()
    }

    pub fn action(&self) -> ActionType {
        self.action
    }

    pub fn status(&self) -> EnforcementStatus {
        self.status
    }

    pub fn version(&self) -> EnforcementVersion {
        self.version
    }

    pub fn decision_id(&self) -> DecisionId {
        self.decision_id
    }

    pub fn applied_at(&self) -> DateTime<Utc> {
        self.applied_at
    }

    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    pub fn reversed_at(&self) -> Option<DateTime<Utc>> {
        self.reversed_at
    }

    /// Whether the enforcement is in force at `now` (Active and not past expiry).
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.status == EnforcementStatus::Active && !self.has_reached_expiry(now)
    }

    fn has_reached_expiry(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|exp| now >= exp)
    }

    // ─── Commands ────────────────────────────────────────────────────────────

    /// Invariant 2: reverses an active enforcement (appeal overturn / re-review),
    /// emitting [`EnforcementReversed`].
    pub fn reverse(&mut self, now: DateTime<Utc>, correlation_id: Uuid) -> Result<(), ModerationError> {
        match self.status {
            EnforcementStatus::Reversed => return Err(ModerationError::EnforcementAlreadyReversed),
            EnforcementStatus::Expired => {
                return Err(ModerationError::InvalidEnforcementTransition {
                    from: self.status.to_string(),
                    to: EnforcementStatus::Reversed.to_string(),
                });
            }
            EnforcementStatus::Active => {}
        }
        self.status = EnforcementStatus::Reversed;
        self.reversed_at = Some(now);
        self.pending_events.push(DomainEvent::EnforcementReversed(EnforcementReversed {
            enforcement_id: self.id,
            subject: self.subject.clone(),
            actor_id: self.subject.actor_id(),
            version: self.version,
            occurred_at: now,
            correlation_id,
        }));
        Ok(())
    }

    /// Invariant 3: marks an active, past-expiry enforcement `Expired`. No event —
    /// expiry is the absence of enforcement, not a fact to react to.
    pub fn mark_expired(&mut self, now: DateTime<Utc>) -> Result<(), ModerationError> {
        if self.status != EnforcementStatus::Active {
            return Err(ModerationError::InvalidEnforcementTransition {
                from: self.status.to_string(),
                to: EnforcementStatus::Expired.to_string(),
            });
        }
        if !self.has_reached_expiry(now) {
            return Err(ModerationError::DomainViolation {
                field: "enforcement".into(),
                message: "enforcement has not reached its expiry".into(),
            });
        }
        self.status = EnforcementStatus::Expired;
        Ok(())
    }

    /// Drains accumulated events for the unit-of-work to publish.
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_object::EntityType;
    use chrono::Duration;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn subject() -> SubjectRef {
        SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(1)), "feed").unwrap()
    }

    fn params(action: ActionType, expires_at: Option<DateTime<Utc>>) -> EnforcementParams {
        EnforcementParams {
            subject: subject(),
            action,
            decision_id: DecisionId::new(),
            version: EnforcementVersion::INITIAL,
            applied_at: t0(),
            expires_at,
            correlation_id: Uuid::now_v7(),
        }
    }

    #[test]
    fn apply_starts_active_and_emits_event() {
        let mut e = EnforcementAction::apply(params(ActionType::RemoveContent, None)).unwrap();
        assert_eq!(e.status(), EnforcementStatus::Active);
        assert!(e.is_active(t0()));
        let events = e.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "moderation.enforcement_applied");
        assert_eq!(events[0].actor_id(), subject().actor_id());
        assert!(e.drain_events().is_empty(), "events drain once");
    }

    #[test]
    fn no_action_cannot_be_enforced() {
        assert!(matches!(
            EnforcementAction::apply(params(ActionType::NoAction, None)).unwrap_err(),
            ModerationError::DomainViolation { .. }
        ));
    }

    #[test]
    fn expiry_must_be_after_applied() {
        let bad = Some(t0() - Duration::seconds(1));
        assert!(matches!(
            EnforcementAction::apply(params(ActionType::RestrictActor, bad)).unwrap_err(),
            ModerationError::DomainViolation { .. }
        ));
    }

    #[test]
    fn reverse_emits_event_and_is_terminal() {
        let mut e = EnforcementAction::apply(params(ActionType::Suspend, None)).unwrap();
        e.drain_events();
        e.reverse(t0() + Duration::hours(1), Uuid::now_v7()).unwrap();
        assert_eq!(e.status(), EnforcementStatus::Reversed);
        let events = e.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "moderation.enforcement_reversed");
        // double reverse rejected
        assert!(matches!(
            e.reverse(t0(), Uuid::now_v7()).unwrap_err(),
            ModerationError::EnforcementAlreadyReversed
        ));
    }

    #[test]
    fn time_boxed_enforcement_becomes_inactive_then_expirable() {
        let exp = t0() + Duration::hours(24);
        let mut e = EnforcementAction::apply(params(ActionType::RestrictActor, Some(exp))).unwrap();
        assert!(e.is_active(t0()));
        assert!(!e.is_active(exp), "inactive once expiry reached");
        // cannot mark expired before expiry
        assert!(e.mark_expired(t0()).is_err());
        // can after
        e.mark_expired(exp).unwrap();
        assert_eq!(e.status(), EnforcementStatus::Expired);
    }

    #[test]
    fn cannot_reverse_an_expired_enforcement() {
        let exp = t0() + Duration::hours(1);
        let mut e = EnforcementAction::apply(params(ActionType::RestrictActor, Some(exp))).unwrap();
        e.mark_expired(exp).unwrap();
        assert!(matches!(
            e.reverse(exp, Uuid::now_v7()).unwrap_err(),
            ModerationError::InvalidEnforcementTransition { .. }
        ));
    }
}
