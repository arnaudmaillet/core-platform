use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use uuid::Uuid;

use crate::application::port::MemberRepository;
use crate::domain::value_object::{ConversationId, ProfileId, Role};
use crate::error::ChatError;

/// Read projection of a roster entry.
#[derive(Debug, Clone)]
pub struct MemberView {
    pub profile_id:   Uuid,
    pub role:         Role,
    pub joined_at_ms: i64,
    pub last_read:    Option<Uuid>,
}

/// Lists the bounded Member-Plane roster. Only members may view the roster, so a
/// requester that is not a member is denied. The roster is bounded, so the
/// result is unpaginated.
pub struct ListMembersQuery {
    pub conversation_id: String,
    pub requester_id:    String,
}

impl Query for ListMembersQuery {
    type Response = Vec<MemberView>;
}

pub struct ListMembersHandler<MR> {
    pub member_repo: Arc<MR>,
}

impl<MR> QueryHandler<ListMembersQuery> for ListMembersHandler<MR>
where
    MR: MemberRepository,
{
    type Error = ChatError;

    async fn handle(
        &self,
        envelope: Envelope<ListMembersQuery>,
    ) -> Result<Vec<MemberView>, ChatError> {
        let q = &envelope.payload;

        let conversation_id = ConversationId::try_from(q.conversation_id.as_str())?;
        let requester_id    = ProfileId::try_from(q.requester_id.as_str())?;

        if self.member_repo.find(&conversation_id, &requester_id).await?.is_none() {
            return Err(ChatError::NotAMember {
                profile_id:      requester_id.as_str(),
                conversation_id: conversation_id.as_str(),
            });
        }

        let members = self.member_repo.list(&conversation_id).await?;

        Ok(members
            .into_iter()
            .map(|p| MemberView {
                profile_id:   p.profile_id().as_uuid(),
                role:         p.role(),
                joined_at_ms: p.joined_at().timestamp_millis(),
                last_read:    p.last_read().map(|m| m.as_uuid()),
            })
            .collect())
    }
}
