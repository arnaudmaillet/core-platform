use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::MemberRepository;
use crate::domain::value_object::{ConversationId, MessageId, ProfileId};
use crate::error::ChatError;

/// Advances a member's read-receipt horizon to `message_id`.
///
/// Read-receipts are a Member-Plane-only concept (O(members)); a non-member
/// (audience) cannot mark read. The horizon is monotone — a stale acknowledgement
/// is a no-op via [`Participant::mark_read`](crate::domain::aggregate::Participant::mark_read).
pub struct MarkReadCommand {
    pub conversation_id: String,
    pub member_id:       String,
    pub message_id:      String,
}

impl Command for MarkReadCommand {}

impl Validate for MarkReadCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-050",
                "conversation_id must not be empty",
            ));
        }
        if self.member_id.trim().is_empty() {
            v.push(FieldViolation::new("member_id", "CHT-VAL-051", "member_id must not be empty"));
        }
        if self.message_id.trim().is_empty() {
            v.push(FieldViolation::new("message_id", "CHT-VAL-052", "message_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct MarkReadHandler<MR> {
    pub member_repo: Arc<MR>,
}

impl<MR> CommandHandler<MarkReadCommand> for MarkReadHandler<MR>
where
    MR: MemberRepository,
{
    type Error = ChatError;

    async fn handle(&self, envelope: Envelope<MarkReadCommand>) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let member_id       = ProfileId::try_from(cmd.member_id.as_str())?;
        let message_id      = MessageId::try_from(cmd.message_id.as_str())?;

        let mut member = self
            .member_repo
            .find(&conversation_id, &member_id)
            .await?
            .ok_or_else(|| ChatError::NotAMember {
                profile_id:      member_id.as_str(),
                conversation_id: conversation_id.as_str(),
            })?;

        // Monotone advance; persist the resulting (possibly unchanged) horizon.
        member.mark_read(message_id);
        if let Some(horizon) = member.last_read() {
            self.member_repo
                .update_last_read(&conversation_id, &member_id, horizon)
                .await?;
        }

        Ok(())
    }
}
