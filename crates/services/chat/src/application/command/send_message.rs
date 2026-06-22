use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{EventPublisher, MemberRepository, MessageRepository};
use crate::domain::aggregate::Message;
use crate::domain::event::{MessageEvent, MessageSentEvent};
use crate::domain::value_object::{
    ContentType, ConversationId, MessageContent, MessageId, ProfileId,
};
use crate::error::ChatError;

/// Posts a message to a conversation.
///
/// The member write path is intentionally lean: a single authorization read
/// (roster membership) followed by the durable write and the fan-out event. The
/// conversation aggregate is not loaded here — membership implies existence — to
/// keep the hot write path to one read + one write.
pub struct SendMessageCommand {
    pub message_id:      String,
    pub conversation_id: String,
    pub sender_id:       String,
    pub content_type:    i32,
    pub body:            String,
    pub media_ref:       Option<String>,
    pub reply_to:        Option<String>,
}

impl Command for SendMessageCommand {}

impl Validate for SendMessageCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.message_id.trim().is_empty() {
            v.push(FieldViolation::new("message_id", "CHT-VAL-010", "message_id must not be empty"));
        }
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-011",
                "conversation_id must not be empty",
            ));
        }
        if self.sender_id.trim().is_empty() {
            v.push(FieldViolation::new("sender_id", "CHT-VAL-012", "sender_id must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct SendMessageHandler<MR, MSG, EP> {
    pub member_repo:  Arc<MR>,
    pub message_repo: Arc<MSG>,
    pub publisher:    Arc<EP>,
}

impl<MR, MSG, EP> CommandHandler<SendMessageCommand> for SendMessageHandler<MR, MSG, EP>
where
    MR:  MemberRepository,
    MSG: MessageRepository,
    EP:  EventPublisher,
{
    type Error = ChatError;

    async fn handle(&self, envelope: Envelope<SendMessageCommand>) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let sender_id       = ProfileId::try_from(cmd.sender_id.as_str())?;
        let message_id      = MessageId::try_from(cmd.message_id.as_str())?;
        let content_type    = ContentType::try_from(cmd.content_type as i8)?;

        // Authorization: only roster members may write. Audience roles are never
        // in the roster, so this single read enforces read-only for guests.
        let member = self
            .member_repo
            .find(&conversation_id, &sender_id)
            .await?
            .ok_or_else(|| ChatError::NotAMember {
                profile_id:      sender_id.as_str(),
                conversation_id: conversation_id.as_str(),
            })?;

        if !member.can_write() {
            return Err(ChatError::NotAuthorized {
                profile_id:      sender_id.as_str(),
                conversation_id: conversation_id.as_str(),
            });
        }

        let reply_to = cmd
            .reply_to
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(MessageId::try_from)
            .transpose()?;

        let message = Message::create(
            message_id,
            conversation_id,
            sender_id,
            content_type,
            MessageContent::new(cmd.body.clone())?,
            cmd.media_ref.clone().filter(|s| !s.is_empty()),
            reply_to,
        )?;

        // Durable write first; the event is the seam the routing layer forks into
        // the Member-Plane broadcast and the Audience-Plane shadow.
        self.message_repo.insert(&message).await?;

        let event = MessageEvent::Sent(MessageSentEvent {
            conversation_id: conversation_id.as_str(),
            message_id:      message.id().as_str(),
            sender_id:       sender_id.as_str(),
            content_type:    content_type.as_str().to_owned(),
            body:            message.content().as_str().to_owned(),
            media_ref:       message.media_ref().map(str::to_owned),
            reply_to:        message.reply_to().map(|m| m.as_str()),
            created_at_ms:   message.created_at().timestamp_millis(),
        });
        self.publisher.publish_message(&event).await?;

        Ok(())
    }
}
