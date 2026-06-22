use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{ConversationRepository, EventPublisher, MemberRepository};
use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;

/// Toggles a conversation's visibility (the `Private` <-> `Public` switch).
///
/// `make_public = true` attaches the Audience Plane and stamps the public-since
/// watermark; `false` detaches it. Authorization requires an administering role
/// (owner/admin); the aggregate enforces the monotone transition guard.
pub struct ToggleVisibilityCommand {
    pub conversation_id: String,
    pub actor_id:        String,
    pub make_public:     bool,
}

impl Command for ToggleVisibilityCommand {}

impl Validate for ToggleVisibilityCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-020",
                "conversation_id must not be empty",
            ));
        }
        if self.actor_id.trim().is_empty() {
            v.push(FieldViolation::new("actor_id", "CHT-VAL-021", "actor_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct ToggleVisibilityHandler<CR, MR, EP> {
    pub conversation_repo: Arc<CR>,
    pub member_repo:       Arc<MR>,
    pub publisher:         Arc<EP>,
}

impl<CR, MR, EP> CommandHandler<ToggleVisibilityCommand>
    for ToggleVisibilityHandler<CR, MR, EP>
where
    CR: ConversationRepository,
    MR: MemberRepository,
    EP: EventPublisher,
{
    type Error = ChatError;

    async fn handle(
        &self,
        envelope: Envelope<ToggleVisibilityCommand>,
    ) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let actor_id        = ProfileId::try_from(cmd.actor_id.as_str())?;

        let mut conversation = self
            .conversation_repo
            .find(&conversation_id)
            .await?
            .ok_or_else(|| ChatError::ConversationNotFound {
                conversation_id: conversation_id.as_str(),
            })?;

        // Authorization: the actor must be an administering member.
        let actor = self
            .member_repo
            .find(&conversation_id, &actor_id)
            .await?
            .ok_or_else(|| ChatError::NotAMember {
                profile_id:      actor_id.as_str(),
                conversation_id: conversation_id.as_str(),
            })?;

        if !actor.can_administer() {
            return Err(ChatError::NotAuthorized {
                profile_id:      actor_id.as_str(),
                conversation_id: conversation_id.as_str(),
            });
        }

        if cmd.make_public {
            conversation.publish()?;
        } else {
            conversation.unpublish()?;
        }

        self.conversation_repo.update(&conversation).await?;

        for event in conversation.take_events() {
            self.publisher.publish_conversation(&event).await?;
        }

        Ok(())
    }
}
