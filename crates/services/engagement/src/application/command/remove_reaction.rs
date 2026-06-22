use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EngagementEventPublisher, ScoreStore};
use crate::domain::aggregate::Reaction;
use crate::domain::event::reaction_event::ReactionKafkaEvent;
use crate::domain::value_object::{PostId, ProfileId};
use crate::error::EngagementError;

pub struct RemoveReactionCommand {
    pub post_id:    String,
    pub profile_id: String,
}

impl Command for RemoveReactionCommand {}

impl Validate for RemoveReactionCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "ENG-VAL-001", "post_id must not be empty"));
        }
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "ENG-VAL-002", "profile_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct RemoveReactionHandler<S, P> {
    pub score_store: Arc<S>,
    pub publisher:   Arc<P>,
}

impl<S, P> CommandHandler<RemoveReactionCommand> for RemoveReactionHandler<S, P>
where
    S: ScoreStore,
    P: EngagementEventPublisher,
{
    type Error = EngagementError;

    async fn handle(&self, envelope: Envelope<RemoveReactionCommand>) -> Result<(), EngagementError> {
        let cmd = &envelope.payload;

        let post_id    = PostId::try_from(cmd.post_id.as_str())?;
        let profile_id = ProfileId::try_from(cmd.profile_id.as_str())?;

        let removed = self.score_store
            .atomic_remove_reaction(&post_id, &profile_id)
            .await?;

        let (kind, weight) = removed.ok_or_else(|| EngagementError::ReactionNotFound {
            post_id:    post_id.as_str(),
            profile_id: profile_id.as_str(),
        })?;

        let event = Reaction::build_removed_event(&post_id, &profile_id, kind, weight);

        self.publisher
            .publish_reaction_event(&ReactionKafkaEvent::Removed(event))
            .await?;

        tracing::debug!(
            post_id    = %post_id,
            profile_id = %profile_id,
            kind       = kind.as_redis_key(),
            "reaction removed"
        );

        Ok(())
    }
}
