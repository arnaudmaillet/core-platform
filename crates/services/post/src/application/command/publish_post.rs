use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::{
    application::port::{AuthorTierStore, EventPublisher, PostRepository},
    domain::{event::DomainEvent, value_object::{PostId, ProfileId}},
    error::PostError,
};

pub struct PublishPostCommand {
    pub post_id:    String,
    pub profile_id: String,
}

impl Command for PublishPostCommand {}

impl Validate for PublishPostCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "PST-VAL-001", "post_id must not be empty"));
        }
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "PST-VAL-002", "profile_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct PublishPostHandler<R, P> {
    pub repository:        Arc<R>,
    pub publisher:         Arc<P>,
    pub author_tier_store: Arc<dyn AuthorTierStore>,
}

impl<R, P> CommandHandler<PublishPostCommand> for PublishPostHandler<R, P>
where
    R: PostRepository,
    P: EventPublisher,
{
    type Error = PostError;

    async fn handle(&self, envelope: Envelope<PublishPostCommand>) -> Result<(), PostError> {
        let cmd = &envelope.payload;

        let post_id    = PostId::try_from(cmd.post_id.as_str())?;
        let profile_id = ProfileId::try_from(cmd.profile_id.as_str())?;

        let mut post = self.repository.find_by_id(&post_id).await?
            .ok_or_else(|| PostError::PostNotFound { post_id: post_id.as_str() })?;

        if post.profile_id().as_uuid() != profile_id.as_uuid() {
            return Err(PostError::AuthorMismatch {
                post_id:   post_id.as_str(),
                caller_id: profile_id.as_str(),
            });
        }

        post.publish()?;
        self.repository.update_lifecycle(&post).await?;

        // Stamp the author's current tier (denormalized from profile.v1.events)
        // onto the published event so timeline routes VIP authors to its read path.
        // A read failure degrades to Standard rather than blocking the publish.
        let author_tier = match self.author_tier_store.get_tier(&profile_id).await {
            Ok(tier) => tier,
            Err(error) => {
                tracing::warn!(%error, "author-tier read failed at publish; defaulting to Standard");
                0
            }
        };

        for mut event in post.take_events() {
            if let DomainEvent::PostPublished(ref mut e) = event {
                e.author_tier = author_tier;
            }
            self.publisher.publish(&event).await?;
        }
        Ok(())
    }
}
