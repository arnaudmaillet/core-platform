use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, SocialGraphCache, SocialGraphRepository};
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

#[derive(Debug, Clone)]
pub struct UnblockProfileCommand {
    pub actor_id:  String,
    pub target_id: String,
}

impl Command for UnblockProfileCommand {}

impl Validate for UnblockProfileCommand {
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

pub struct UnblockProfileHandler {
    repo:      Arc<dyn SocialGraphRepository>,
    cache:     Arc<dyn SocialGraphCache>,
    #[allow(dead_code)]
    publisher: Arc<dyn EventPublisher>,
}

impl UnblockProfileHandler {
    pub fn new(
        repo:      Arc<dyn SocialGraphRepository>,
        cache:     Arc<dyn SocialGraphCache>,
        publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self { repo, cache, publisher }
    }
}

impl CommandHandler<UnblockProfileCommand> for UnblockProfileHandler {
    type Error = SocialGraphError;

    async fn handle(
        &self,
        envelope: Envelope<UnblockProfileCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let actor_id  = ProfileId::try_from(cmd.actor_id.as_str())?;
        let target_id = ProfileId::try_from(cmd.target_id.as_str())?;

        let mut relation = self.repo.load_relation(&actor_id, &target_id).await?;

        // Domain guard: must be blocking (returns NotBlocked if not).
        relation.unblock()?;

        self.repo.delete_block(&actor_id, &target_id).await?;

        let _ = self.cache.remove_block(&actor_id, &target_id).await;

        // ProfileUnblocked is intentionally not published to Kafka.
        // Unblocking has no downstream fan-out consequence for timeline engines.
        let _ = relation.take_events();

        Ok(())
    }
}
