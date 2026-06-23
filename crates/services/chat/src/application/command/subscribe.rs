use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{ConversationRepository, SubscriptionRepository};
use crate::domain::value_object::{ConversationId, ProfileId};
use crate::error::ChatError;

/// Subscribes a profile to the Audience Plane of a public conversation.
///
/// Audience subscription is a read-side concern: it never touches the aggregate
/// roster, so it carries no roster cap and emits no lifecycle event. It only
/// requires the conversation to be public.
pub struct SubscribeCommand {
    pub conversation_id: String,
    pub subscriber_id:   String,
}

impl Command for SubscribeCommand {}

impl Validate for SubscribeCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-040",
                "conversation_id must not be empty",
            ));
        }
        if self.subscriber_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "subscriber_id",
                "CHT-VAL-041",
                "subscriber_id must not be empty",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct SubscribeHandler<CR, SR> {
    pub conversation_repo: Arc<CR>,
    pub subscription_repo: Arc<SR>,
}

impl<CR, SR> CommandHandler<SubscribeCommand> for SubscribeHandler<CR, SR>
where
    CR: ConversationRepository,
    SR: SubscriptionRepository,
{
    type Error = ChatError;

    async fn handle(&self, envelope: Envelope<SubscribeCommand>) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let subscriber_id   = ProfileId::try_from(cmd.subscriber_id.as_str())?;

        let conversation = self
            .conversation_repo
            .find(&conversation_id)
            .await?
            .ok_or_else(|| ChatError::ConversationNotFound {
                conversation_id: conversation_id.as_str(),
            })?;

        if !conversation.visibility().is_public() {
            return Err(ChatError::ConversationNotPublic {
                conversation_id: conversation_id.as_str(),
            });
        }

        self.subscription_repo.subscribe(&conversation_id, &subscriber_id).await
    }
}

/// Removes an Audience-Plane subscription. Idempotent; requires no visibility
/// check (a profile may always unsubscribe).
pub struct UnsubscribeCommand {
    pub conversation_id: String,
    pub subscriber_id:   String,
}

impl Command for UnsubscribeCommand {}

impl Validate for UnsubscribeCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.conversation_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "conversation_id",
                "CHT-VAL-042",
                "conversation_id must not be empty",
            ));
        }
        if self.subscriber_id.trim().is_empty() {
            v.push(FieldViolation::new(
                "subscriber_id",
                "CHT-VAL-043",
                "subscriber_id must not be empty",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct UnsubscribeHandler<SR> {
    pub subscription_repo: Arc<SR>,
}

impl<SR> CommandHandler<UnsubscribeCommand> for UnsubscribeHandler<SR>
where
    SR: SubscriptionRepository,
{
    type Error = ChatError;

    async fn handle(&self, envelope: Envelope<UnsubscribeCommand>) -> Result<(), ChatError> {
        let cmd = &envelope.payload;

        let conversation_id = ConversationId::try_from(cmd.conversation_id.as_str())?;
        let subscriber_id   = ProfileId::try_from(cmd.subscriber_id.as_str())?;

        self.subscription_repo.unsubscribe(&conversation_id, &subscriber_id).await
    }
}
