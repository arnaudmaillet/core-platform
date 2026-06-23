use async_trait::async_trait;

use crate::domain::aggregate::Participant;
use crate::domain::value_object::{ConversationId, MessageId, ProfileId};
use crate::error::ChatError;

/// Persistence port for the bounded Member Plane roster
/// (`chat.members_by_conversation`).
///
/// All operations are single-partition: the roster is bounded (<= 500), so the
/// full-roster `list` is a safe, token-aware read.
#[async_trait]
pub trait MemberRepository: Send + Sync + 'static {
    /// Adds a participant to the roster.
    async fn insert(
        &self,
        conversation_id: &ConversationId,
        participant:     &Participant,
    ) -> Result<(), ChatError>;

    /// Reads a single participant, or `None` if the profile is not a member.
    /// This is the authorization probe on the write/admin path.
    async fn find(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<Option<Participant>, ChatError>;

    /// Advances a member's read-receipt horizon.
    async fn update_last_read(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
        last_read:       MessageId,
    ) -> Result<(), ChatError>;

    /// Lists the full (bounded) roster.
    async fn list(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<Participant>, ChatError>;

    /// Removes a participant from the roster.
    async fn delete(
        &self,
        conversation_id: &ConversationId,
        member_id:       &ProfileId,
    ) -> Result<(), ChatError>;
}
