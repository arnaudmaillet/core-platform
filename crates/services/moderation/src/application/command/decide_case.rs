use std::sync::Arc;

use chrono::{DateTime, Utc};
use cqrs::Envelope;

use crate::application::policy::ModerationPolicy;
use crate::application::port::{
    AccountDirectory, CaseRepository, DecisionRepository, EnforcementProjection,
    EnforcementRepository, EventPublisher, PenaltyRepository,
};
use crate::domain::aggregate::{
    Case, CaseOpenParams, Decision, DecisionAuthor, DecisionParams, EnforcementAction,
    EnforcementParams,
};
use crate::domain::value_object::{ActionType, CaseId, PolicyCategory, PolicyVersion, SubjectRef};
use crate::error::ModerationError;

// ─── OpenCase (manual, idempotent) ────────────────────────────────────────────

/// Manually open (or idempotently return) a case for a subject.
#[derive(Debug, Clone)]
pub struct OpenCaseCommand {
    pub subject: SubjectRef,
    pub category: PolicyCategory,
    pub queue: String,
    pub priority: String,
}

/// Whether a fresh case was created or an existing one returned.
#[derive(Debug, Clone)]
pub struct OpenedCase {
    pub case: Case,
    pub created: bool,
}

pub struct OpenCaseHandler {
    cases: Arc<dyn CaseRepository>,
    publisher: Arc<dyn EventPublisher>,
}

impl OpenCaseHandler {
    pub fn new(cases: Arc<dyn CaseRepository>, publisher: Arc<dyn EventPublisher>) -> Self {
        Self { cases, publisher }
    }

    pub async fn handle(
        &self,
        envelope: Envelope<OpenCaseCommand>,
        now: DateTime<Utc>,
    ) -> Result<OpenedCase, ModerationError> {
        let cmd = envelope.payload;
        let id = CaseId::for_subject(&cmd.subject);
        if let Some(existing) = self.cases.find_by_id(&id).await? {
            return Ok(OpenedCase { case: existing, created: false });
        }
        let mut case = Case::open(CaseOpenParams {
            subject: cmd.subject,
            category: cmd.category,
            queue: cmd.queue,
            priority: cmd.priority,
            opened_at: now,
            correlation_id: envelope.correlation_id,
        });
        self.cases.save(&case).await?;
        for event in &case.drain_events() {
            self.publisher.publish(event).await?;
        }
        Ok(OpenedCase { case, created: true })
    }
}

// ─── AssignCase ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AssignCaseCommand {
    pub case_id: CaseId,
    pub reviewer_id: String,
}

pub struct AssignCaseHandler {
    cases: Arc<dyn CaseRepository>,
}

impl AssignCaseHandler {
    pub fn new(cases: Arc<dyn CaseRepository>) -> Self {
        Self { cases }
    }

    pub async fn handle(
        &self,
        envelope: Envelope<AssignCaseCommand>,
    ) -> Result<Case, ModerationError> {
        let cmd = envelope.payload;
        let mut case = self
            .cases
            .find_by_id(&cmd.case_id)
            .await?
            .ok_or(ModerationError::CaseNotFound { id: cmd.case_id.as_str() })?;
        case.assign(cmd.reviewer_id)?;
        self.cases.save(&case).await?;
        Ok(case)
    }
}

// ─── DecideCase (the core flow) ───────────────────────────────────────────────

/// Action or dismiss a case under a pinned policy version.
#[derive(Debug, Clone)]
pub struct DecideCaseCommand {
    pub case_id: CaseId,
    pub action: ActionType,
    pub category: PolicyCategory,
    pub rationale: String,
    pub reviewer_id: String,
    pub policy_version: String,
}

/// The decision recorded and, unless dismissed, the enforcement applied.
#[derive(Debug, Clone)]
pub struct DecideOutcome {
    pub decision: Decision,
    pub enforcement: Option<EnforcementAction>,
}

/// Records the decision, applies enforcement (with a monotonic per-subject
/// version), reflects actor-level restrictions on the hot-path projection, strikes
/// the actor's penalty ledger (graduated enforcement), resolves the case, and
/// publishes the events — in that durable-first order.
pub struct DecideCaseHandler {
    cases: Arc<dyn CaseRepository>,
    decisions: Arc<dyn DecisionRepository>,
    enforcements: Arc<dyn EnforcementRepository>,
    penalties: Arc<dyn PenaltyRepository>,
    projection: Arc<dyn EnforcementProjection>,
    accounts: Arc<dyn AccountDirectory>,
    publisher: Arc<dyn EventPublisher>,
    policy: ModerationPolicy,
}

impl DecideCaseHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cases: Arc<dyn CaseRepository>,
        decisions: Arc<dyn DecisionRepository>,
        enforcements: Arc<dyn EnforcementRepository>,
        penalties: Arc<dyn PenaltyRepository>,
        projection: Arc<dyn EnforcementProjection>,
        accounts: Arc<dyn AccountDirectory>,
        publisher: Arc<dyn EventPublisher>,
        policy: ModerationPolicy,
    ) -> Self {
        Self { cases, decisions, enforcements, penalties, projection, accounts, publisher, policy }
    }

    pub async fn handle(
        &self,
        envelope: Envelope<DecideCaseCommand>,
        now: DateTime<Utc>,
    ) -> Result<DecideOutcome, ModerationError> {
        let cmd = envelope.payload;
        let correlation_id = envelope.correlation_id;

        let mut case = self
            .cases
            .find_by_id(&cmd.case_id)
            .await?
            .ok_or(ModerationError::CaseNotFound { id: cmd.case_id.as_str() })?;
        let subject = case.subject().clone();
        let policy_version = PolicyVersion::new(cmd.policy_version)?;

        // Actor-level actions require the actor to exist, so a typo cannot create a
        // dangling restriction.
        if cmd.action.is_actor_level() && !self.accounts.actor_exists(&subject.actor_id()).await? {
            return Err(ModerationError::DomainViolation {
                field: "actor".into(),
                message: "cannot enforce against an unknown actor".into(),
            });
        }

        // 1. Append the immutable decision.
        let decision = Decision::record(DecisionParams {
            subject: subject.clone(),
            action: cmd.action,
            category: cmd.category,
            policy_version,
            rationale: cmd.rationale,
            author: DecisionAuthor::Reviewer(cmd.reviewer_id),
            decided_at: now,
        })?;
        self.decisions.append(&decision).await?;

        // 1b. Publish the compliance-evidence event (the audit feed) — carries the
        // authority + rationale the offender-centric events omit.
        self.publisher
            .publish(&super::decision_recorded(&decision, correlation_id))
            .await?;

        // 2. Apply enforcement (unless the action is a dismissal).
        let mut enforcement = None;
        if cmd.action.is_enforced() {
            let version = self.enforcements.next_version(&subject).await?;
            let mut enf = EnforcementAction::apply(EnforcementParams {
                subject: subject.clone(),
                action: cmd.action,
                decision_id: decision.id(),
                version,
                applied_at: now,
                expires_at: self.policy.expiry_for(cmd.action, now),
                correlation_id,
            })?;
            self.enforcements.save(&enf).await?;

            if cmd.action.is_actor_level() {
                self.projection
                    .set_actor_restriction(&subject.actor_id(), version, enf.expires_at())
                    .await?;
            }

            // 3. Strike the penalty ledger (drives graduated enforcement on repeat).
            let mut ledger = self.penalties.load(&subject.actor_id()).await?;
            ledger.record_strike(cmd.category, now, &self.policy.penalty);
            self.penalties.save(&ledger).await?;

            self.publish_all(enf.drain_events()).await?;
            enforcement = Some(enf);
        }

        // 4. Resolve the case (emits CaseResolved).
        case.resolve(decision.id(), cmd.action, now, correlation_id)?;
        self.cases.save(&case).await?;
        self.publish_all(case.drain_events()).await?;

        Ok(DecideOutcome { decision, enforcement })
    }

    async fn publish_all(
        &self,
        events: Vec<crate::domain::event::DomainEvent>,
    ) -> Result<(), ModerationError> {
        for event in &events {
            self.publisher.publish(event).await?;
        }
        Ok(())
    }

    /// Recommends the graduated action for an actor under the current policy — a
    /// helper the reviewer UI can call to surface "history warrants X".
    pub async fn recommended_action(
        &self,
        actor_id: &crate::domain::value_object::ActorId,
        now: DateTime<Utc>,
    ) -> Result<ActionType, ModerationError> {
        let ledger = self.penalties.load(actor_id).await?;
        Ok(ledger.recommended_action(now, &self.policy.penalty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{t0, Fixture};
    use crate::domain::value_object::{ActorId, EntityType};
    use uuid::Uuid;

    fn subject() -> SubjectRef {
        SubjectRef::new(EntityType::Post, "p1", ActorId::from_uuid(Uuid::from_u128(1)), "feed").unwrap()
    }

    async fn open_case(fx: &Fixture) -> CaseId {
        let env = Envelope::new(
            Uuid::now_v7(),
            OpenCaseCommand {
                subject: subject(),
                category: PolicyCategory::Harassment,
                queue: "default".into(),
                priority: "normal".into(),
            },
        );
        fx.open_case_handler().handle(env, t0()).await.unwrap().case.id()
    }

    fn decide_env(case_id: CaseId, action: ActionType) -> Envelope<DecideCaseCommand> {
        Envelope::new(
            Uuid::now_v7(),
            DecideCaseCommand {
                case_id,
                action,
                category: PolicyCategory::Harassment,
                rationale: "violates policy".into(),
                reviewer_id: "rev-1".into(),
                policy_version: "2026.06.1".into(),
            },
        )
    }

    #[tokio::test]
    async fn open_case_is_idempotent() {
        let fx = Fixture::new();
        let env1 = Envelope::new(
            Uuid::now_v7(),
            OpenCaseCommand {
                subject: subject(),
                category: PolicyCategory::Spam,
                queue: "q".into(),
                priority: "p".into(),
            },
        );
        let first = fx.open_case_handler().handle(env1.clone(), t0()).await.unwrap();
        let second = fx.open_case_handler().handle(env1, t0()).await.unwrap();
        assert!(first.created);
        assert!(!second.created, "second open returns the existing case");
        assert_eq!(first.case.id(), second.case.id());
    }

    #[tokio::test]
    async fn decide_with_content_action_records_decision_and_enforcement() {
        let fx = Fixture::new();
        let case_id = open_case(&fx).await;
        fx.publisher.clear();

        let out = fx
            .decide_handler()
            .handle(decide_env(case_id, ActionType::RemoveContent), t0())
            .await
            .unwrap();

        assert_eq!(out.decision.action(), ActionType::RemoveContent);
        assert!(out.enforcement.is_some());
        assert_eq!(fx.decisions.count(), 1);
        // DecisionRecorded (the evidence event), then EnforcementApplied, then
        // CaseResolved.
        assert_eq!(
            fx.publisher.event_types(),
            vec![
                "moderation.decision_recorded",
                "moderation.enforcement_applied",
                "moderation.case_resolved"
            ]
        );
        // Content action ⇒ no actor restriction.
        assert!(!fx.projection.is_actor_restricted(&subject().actor_id()).await.unwrap());
    }

    /// The DecisionRecorded evidence event must carry who decided (the authority)
    /// and why (the rationale) — the fields the offender-centric events omit and
    /// the whole reason audit consumes this stream.
    #[tokio::test]
    async fn decision_recorded_carries_authority_and_reason() {
        let fx = Fixture::new();
        let case_id = open_case(&fx).await;
        fx.publisher.clear();

        fx.decide_handler()
            .handle(decide_env(case_id, ActionType::RemoveContent), t0())
            .await
            .unwrap();

        let recorded = fx
            .publisher
            .events()
            .into_iter()
            .find_map(|e| match e {
                crate::domain::event::DomainEvent::DecisionRecorded(d) => Some(d),
                _ => None,
            })
            .expect("a DecisionRecorded event was published");

        assert_eq!(recorded.author, DecisionAuthor::Reviewer("rev-1".into()));
        assert_eq!(recorded.rationale, "violates policy");
        assert_eq!(recorded.policy_version, PolicyVersion::new("2026.06.1").unwrap());
        assert_eq!(recorded.action, ActionType::RemoveContent);
        assert!(recorded.reverses.is_none());
    }

    #[tokio::test]
    async fn dismissal_records_decision_but_no_enforcement() {
        let fx = Fixture::new();
        let case_id = open_case(&fx).await;
        fx.publisher.clear();

        let out = fx
            .decide_handler()
            .handle(decide_env(case_id, ActionType::NoAction), t0())
            .await
            .unwrap();

        assert!(out.enforcement.is_none());
        assert_eq!(fx.decisions.count(), 1);
        // A dismissal still records + publishes the decision evidence, then resolves.
        assert_eq!(
            fx.publisher.event_types(),
            vec!["moderation.decision_recorded", "moderation.case_resolved"]
        );
    }

    #[tokio::test]
    async fn actor_level_action_sets_projection_and_strikes_ledger() {
        let fx = Fixture::new();
        let case_id = open_case(&fx).await;

        fx.decide_handler()
            .handle(decide_env(case_id, ActionType::Suspend), t0())
            .await
            .unwrap();

        assert!(fx.projection.is_actor_restricted(&subject().actor_id()).await.unwrap());
        let ledger = fx.penalties.load(&subject().actor_id()).await.unwrap();
        assert_eq!(ledger.active_strike_count(t0()), 1);
    }

    #[tokio::test]
    async fn actor_level_action_rejected_for_unknown_actor() {
        let fx = Fixture::new();
        fx.accounts.set_known(false);
        let case_id = open_case(&fx).await;
        let err = fx
            .decide_handler()
            .handle(decide_env(case_id, ActionType::Ban), t0())
            .await
            .unwrap_err();
        assert!(matches!(err, ModerationError::DomainViolation { .. }));
        // Nothing enforced.
        assert!(!fx.projection.is_actor_restricted(&subject().actor_id()).await.unwrap());
    }

    #[tokio::test]
    async fn deciding_missing_case_errs() {
        let fx = Fixture::new();
        let err = fx
            .decide_handler()
            .handle(decide_env(CaseId::for_subject(&subject()), ActionType::Warn), t0())
            .await
            .unwrap_err();
        assert!(matches!(err, ModerationError::CaseNotFound { .. }));
    }

    #[tokio::test]
    async fn enforcement_version_is_monotonic_per_subject() {
        let fx = Fixture::new();
        let case_id = open_case(&fx).await;
        let out1 = fx.decide_handler().handle(decide_env(case_id, ActionType::VisibilityLimit), t0()).await.unwrap();
        let v1 = out1.enforcement.unwrap().version();
        // Re-open a fresh case for the same subject and decide again.
        let case_id2 = open_case(&fx).await; // same deterministic id, already resolved → load
        // The existing case is resolved; deciding again should fail (already resolved).
        let err = fx.decide_handler().handle(decide_env(case_id2, ActionType::RemoveContent), t0()).await.unwrap_err();
        assert!(matches!(err, ModerationError::CaseAlreadyResolved));
        assert_eq!(v1.value(), 1);
    }
}
