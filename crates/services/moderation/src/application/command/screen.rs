use std::sync::Arc;

use chrono::{DateTime, Utc};
use cqrs::Envelope;

use crate::application::policy::ModerationPolicy;
use crate::application::port::{
    ContentHash, DecisionRepository, EnforcementProjection, EnforcementRepository, EventPublisher,
    PenaltyRepository, ScreenCorpus,
};
use crate::domain::aggregate::{
    Decision, DecisionAuthor, DecisionParams, EnforcementAction, EnforcementParams,
};
use crate::domain::value_object::{ActionType, EnforcementId, PolicyCategory, SubjectRef};
use crate::error::ModerationError;

/// Plane C — the synchronous, fail-closed pre-publish screen.
#[derive(Debug, Clone)]
pub struct ScreenCommand {
    pub subject: SubjectRef,
    pub hashes: Vec<ContentHash>,
    /// Optional short text for the critical-term blocklist. Transient — never
    /// persisted.
    pub text: Option<String>,
    /// Categories to screen. Empty ⇒ the corpus screens all zero-tolerance ones.
    pub categories: Vec<PolicyCategory>,
}

/// The screen result. `Block` means a known-bad match was found; the caller's
/// per-category fail policy turns that (and any `ScreenUnavailable` error) into a
/// hard pre-publish block. `Allow` means "no known-bad match", never "approved".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenVerdict {
    Allow,
    Block,
    Review,
}

/// The outcome of a screen, including the automated evidence recorded on a match.
#[derive(Debug, Clone)]
pub struct ScreenOutcome {
    pub verdict: ScreenVerdict,
    pub matched_categories: Vec<PolicyCategory>,
    pub match_reference: Option<String>,
    /// The automated enforcement opened on a match (content removal).
    pub enforcement_id: Option<EnforcementId>,
}

/// Screens content and, on a known-bad match, records the automated evidence: an
/// append-only [`Decision`] (authored by the screen rule), a content
/// `RemoveContent` [`EnforcementAction`], a strike against the actor (so the
/// graduated engine escalates), and the `EnforcementApplied` event. A corpus error
/// propagates as `ScreenUnavailable`/`HashCorpusUnavailable` — the caller fails
/// closed.
pub struct ScreenHandler {
    corpus: Arc<dyn ScreenCorpus>,
    decisions: Arc<dyn DecisionRepository>,
    enforcements: Arc<dyn EnforcementRepository>,
    penalties: Arc<dyn PenaltyRepository>,
    projection: Arc<dyn EnforcementProjection>,
    publisher: Arc<dyn EventPublisher>,
    policy: ModerationPolicy,
}

impl ScreenHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        corpus: Arc<dyn ScreenCorpus>,
        decisions: Arc<dyn DecisionRepository>,
        enforcements: Arc<dyn EnforcementRepository>,
        penalties: Arc<dyn PenaltyRepository>,
        projection: Arc<dyn EnforcementProjection>,
        publisher: Arc<dyn EventPublisher>,
        policy: ModerationPolicy,
    ) -> Self {
        Self { corpus, decisions, enforcements, penalties, projection, publisher, policy }
    }

    pub async fn handle(
        &self,
        envelope: Envelope<ScreenCommand>,
        now: DateTime<Utc>,
    ) -> Result<ScreenOutcome, ModerationError> {
        let cmd = envelope.payload;
        let correlation_id = envelope.correlation_id;

        // Hard timeout: a slow/stuck corpus must not wedge the publish path. On
        // elapse we surface `ScreenUnavailable` and the caller fails closed for
        // catastrophic categories.
        let lookup = self.corpus.screen(&cmd.hashes, cmd.text.as_deref(), &cmd.categories);
        let hit = match tokio::time::timeout(self.policy.screen_timeout, lookup).await {
            Ok(result) => result?,
            Err(_elapsed) => return Err(ModerationError::ScreenUnavailable),
        };

        let Some(m) = hit else {
            return Ok(ScreenOutcome {
                verdict: ScreenVerdict::Allow,
                matched_categories: Vec::new(),
                match_reference: None,
                enforcement_id: None,
            });
        };

        // The matched category drives the recorded decision; pick the first (the
        // corpus returns the categories this content is known-bad for).
        let category = m.categories.first().copied().unwrap_or(PolicyCategory::Other);

        // 1. Append the automated decision (the legal evidence record).
        let decision = Decision::record(DecisionParams {
            subject: cmd.subject.clone(),
            action: ActionType::RemoveContent,
            category,
            policy_version: self.policy.screen_policy_version.clone(),
            rationale: format!("automated screen match: {}", m.reference),
            author: DecisionAuthor::Rule("screen:hash-match".into()),
            decided_at: now,
        })?;
        self.decisions.append(&decision).await?;

        // 2. Apply the content removal enforcement and publish it.
        let version = self.enforcements.next_version(&cmd.subject).await?;
        let mut enforcement = EnforcementAction::apply(EnforcementParams {
            subject: cmd.subject.clone(),
            action: ActionType::RemoveContent,
            decision_id: decision.id(),
            version,
            applied_at: now,
            expires_at: None,
            correlation_id,
        })?;
        let enforcement_id = enforcement.id();
        self.enforcements.save(&enforcement).await?;
        self.publish_all(enforcement.drain_events()).await?;

        // 3. Strike the actor so graduated enforcement escalates on repeat offenders.
        let mut ledger = self.penalties.load(&cmd.subject.actor_id()).await?;
        ledger.record_strike(category, now, &self.policy.penalty);
        // A catastrophic strike may now warrant an actor-level restriction; reflect
        // it on the hot-path projection so the actor can't immediately re-offend.
        if ledger.recommended_action(now, &self.policy.penalty).is_actor_level() {
            self.projection
                .set_actor_restriction(&cmd.subject.actor_id(), version, None)
                .await?;
        }
        self.penalties.save(&ledger).await?;

        Ok(ScreenOutcome {
            verdict: ScreenVerdict::Block,
            matched_categories: m.categories,
            match_reference: Some(m.reference),
            enforcement_id: Some(enforcement_id),
        })
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{t0, Fixture};
    use crate::domain::value_object::{ActorId, EntityType};
    use uuid::Uuid;

    fn subject() -> SubjectRef {
        SubjectRef::new(EntityType::Media, "m1", ActorId::from_uuid(Uuid::from_u128(1)), "upload").unwrap()
    }

    fn env(cmd: ScreenCommand) -> Envelope<ScreenCommand> {
        Envelope::new(Uuid::now_v7(), cmd)
    }

    fn cmd() -> ScreenCommand {
        ScreenCommand {
            subject: subject(),
            hashes: vec![ContentHash { algorithm: "pdq".into(), value: "abc".into() }],
            text: None,
            categories: vec![PolicyCategory::Csam],
        }
    }

    #[tokio::test]
    async fn clean_content_allows_and_records_nothing() {
        let fx = Fixture::new();
        let out = fx.screen_handler().handle(env(cmd()), t0()).await.unwrap();
        assert_eq!(out.verdict, ScreenVerdict::Allow);
        assert!(out.enforcement_id.is_none());
        assert_eq!(fx.publisher.count(), 0);
        assert_eq!(fx.decisions.count(), 0);
    }

    #[tokio::test]
    async fn known_bad_match_blocks_and_records_evidence() {
        let fx = Fixture::new();
        fx.corpus.add_known_bad("abc", vec![PolicyCategory::Csam], "ncmec:123");

        let out = fx.screen_handler().handle(env(cmd()), t0()).await.unwrap();

        assert_eq!(out.verdict, ScreenVerdict::Block);
        assert_eq!(out.matched_categories, vec![PolicyCategory::Csam]);
        assert_eq!(out.match_reference.as_deref(), Some("ncmec:123"));
        assert!(out.enforcement_id.is_some());
        // One append-only decision + one EnforcementApplied event.
        assert_eq!(fx.decisions.count(), 1);
        assert_eq!(fx.publisher.event_types(), vec!["moderation.enforcement_applied"]);
        // CSAM weight (6) hits the Ban tier ⇒ actor restricted on the projection.
        assert!(fx.projection.is_actor_restricted(&subject().actor_id()).await.unwrap());
    }

    #[tokio::test]
    async fn corpus_outage_propagates_as_screen_unavailable() {
        let fx = Fixture::new();
        fx.corpus.set_unavailable();
        let err = fx.screen_handler().handle(env(cmd()), t0()).await.unwrap_err();
        assert!(matches!(err, ModerationError::ScreenUnavailable));
    }

    #[tokio::test]
    async fn slow_corpus_trips_the_hard_timeout() {
        let mut fx = Fixture::new();
        fx.policy.screen_timeout = std::time::Duration::from_millis(10);
        fx.corpus.set_delay(std::time::Duration::from_millis(150));

        let err = fx.screen_handler().handle(env(cmd()), t0()).await.unwrap_err();
        assert!(
            matches!(err, ModerationError::ScreenUnavailable),
            "a corpus slower than the timeout must fail closed, not hang"
        );
        // Nothing recorded when the gate times out.
        assert_eq!(fx.decisions.count(), 0);
        assert_eq!(fx.publisher.count(), 0);
    }
}
