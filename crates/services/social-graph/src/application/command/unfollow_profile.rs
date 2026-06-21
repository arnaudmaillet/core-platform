use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, SocialGraphCache, SocialGraphRepository};
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

#[derive(Debug, Clone)]
pub struct UnfollowProfileCommand {
    pub actor_id:  String,
    pub target_id: String,
}

impl Command for UnfollowProfileCommand {}

impl Validate for UnfollowProfileCommand {
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

pub struct UnfollowProfileHandler {
    repo:      Arc<dyn SocialGraphRepository>,
    cache:     Arc<dyn SocialGraphCache>,
    publisher: Arc<dyn EventPublisher>,
}

impl UnfollowProfileHandler {
    pub fn new(
        repo:      Arc<dyn SocialGraphRepository>,
        cache:     Arc<dyn SocialGraphCache>,
        publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self { repo, cache, publisher }
    }
}

impl CommandHandler<UnfollowProfileCommand> for UnfollowProfileHandler {
    type Error = SocialGraphError;

    async fn handle(
        &self,
        envelope: Envelope<UnfollowProfileCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let actor_id  = ProfileId::try_from(cmd.actor_id.as_str())?;
        let target_id = ProfileId::try_from(cmd.target_id.as_str())?;

        let mut relation = self.repo.load_relation(&actor_id, &target_id).await?;

        // `unfollow()` returns the `followed_at` timestamp needed for the DELETE.
        let followed_at = relation.unfollow()?;

        // Triple DELETE: follow_status + following + followers.
        self.repo.delete_follow(&actor_id, &target_id, followed_at).await?;

        let _ = self.cache.remove_following(&actor_id, &target_id).await;
        let _ = self.cache.decr_followers_count(&target_id).await;
        let _ = self.cache.decr_following_count(&actor_id).await;

        for event in relation.take_events() {
            let _ = self.publisher.publish(&event).await;
        }

        Ok(())
    }
}
