use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{ConversationRepository, EventPublisher, MemberRepository};
use crate::domain::aggregate::Participant;
use crate::domain::value_object::{ConversationId, ProfileId, Role};
use crate::error::ChatError;

/// Admits a profile to the bounded Member Plane as a regular `Member`.
///
/// Enforces the roster cap through the aggregate and rejects a profile that is
/// already a member. Promotion to admin/owner is a separate operation.
pub struct JoinAsMemberCommand {
    pub conversation_id: String,
    pub profile_id:      String,
}

impl Command for JoinAsMemberCommand {}

impl Validate for JoinAsMemberCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-030",
                "conversation_id must not be empty",
            ));
        }
        if self.profile_id.trim().is_empty() {
            v.push(FieldViolation::new("profile_id", "CHT-VAL-031", "profile_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct JoinAsMemberHandler<CR, MR, EP> {
    pub conversation_repo: Arc<CR>,
    pub member_repo:       Arc<MR>,
    pub publisher:         Arc<EP>,
}

impl<CR, MR, EP> CommandHandler<JoinAsMemberCommand> for JoinAsMemberHandler<CR, MR, EP>
where
    CR: ConversationRepository,
    MR: MemberRepository,
    EP: EventPublisher,
{
    type Error = ChatError;

    async fn handle(&self, envelope: Envelope<JoinAsMemberCommand>) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let profile_id      = ProfileId::try_from(cmd.profile_id.as_str())?;

        let mut conversation = self
            .conversation_repo
            .find(&conversation_id)
            .await?
            .ok_or_else(|| ChatError::ConversationNotFound {
                conversation_id: conversation_id.as_str(),
            })?;

        if self.member_repo.find(&conversation_id, &profile_id).await?.is_some() {
            return Err(ChatError::AlreadyMember {
                profile_id:      profile_id.as_str(),
                conversation_id: conversation_id.as_str(),
            });
        }

        // Roster-cap invariant enforced inside the aggregate.
        conversation.admit_member(profile_id, Role::Member)?;
        let participant = Participant::new(profile_id, Role::Member)?;

        self.member_repo.insert(&conversation_id, &participant).await?;
        self.conversation_repo.update(&conversation).await?;

        for event in conversation.take_events() {
            self.publisher.publish_conversation(&event).await?;
        }

        Ok(())
    }
}
