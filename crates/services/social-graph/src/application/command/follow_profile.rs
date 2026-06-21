use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, SocialGraphCache, SocialGraphRepository};
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

#[derive(Debug, Clone)]
pub struct FollowProfileCommand {
    pub actor_id:  String,
    pub target_id: String,
}

impl Command for FollowProfileCommand {}

impl Validate for FollowProfileCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.actor_id.trim().is_empty() {
            v.push(FieldViolation::new("actor_id", "VAL-4001", "actor_id must not be empty"));
        }
        if self.target_id.trim().is_empty() {
            v.push(FieldViolation::new("target_id", "VAL-4002", "target_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct FollowProfileHandler {
    repo:      Arc<dyn SocialGraphRepository>,
    cache:     Arc<dyn SocialGraphCache>,
    publisher: Arc<dyn EventPublisher>,
}

impl FollowProfileHandler {
    pub fn new(
        repo:      Arc<dyn SocialGraphRepository>,
        cache:     Arc<dyn SocialGraphCache>,
        publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self { repo, cache, publisher }
    }
}

impl CommandHandler<FollowProfileCommand> for FollowProfileHandler {
    type Error = SocialGraphError;

    async fn handle(
        &self,
        envelope: Envelope<FollowProfileCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let actor_id  = ProfileId::try_from(cmd.actor_id.as_str())?;
        let target_id = ProfileId::try_from(cmd.target_id.as_str())?;

        if actor_id == target_id {
            return Err(SocialGraphError::SelfInteraction);
        }

        // Load full bidirectional context (4 concurrent ScyllaDB point-lookups).
        let mut relation = self.repo.load_relation(&actor_id, &target_id).await?;

        // Domain invariant enforcement (block-gate + idempotency guard).
        let followed_at = relation.follow()?;

        // Persist the follow edge across the three adjacency tables.
        self.repo.persist_follow(&actor_id, &target_id, followed_at).await?;

        // Update Redis side-effects (best-effort; errors are logged, not surfaced).
        let _ = self.cache.add_following(&actor_id, &target_id).await;
        let _ = self.cache.incr_followers_count(&target_id).await;
        let _ = self.cache.incr_following_count(&actor_id).await;

        // Publish domain event to Kafka for downstream timeline fan-out engines.
        for event in relation.take_events() {
            let _ = self.publisher.publish(&event).await;
        }

        Ok(())
    }
}
