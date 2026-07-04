use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};
use uuid::Uuid;

use crate::application::port::SubscriptionRepository;
use crate::domain::value_object::ProfileId;
use crate::error::ChatError;

pub struct SubscriptionPage {
    pub conversation_ids: Vec<String>,
    pub next_page_token:  Option<String>,
}

/// Lists the conversations a profile subscribes to (its Audience-Plane
/// memberships), paginated by `conversation_id`.
pub struct ListSubscriptionsQuery {
    pub subscriber_id: String,
    pub limit:         i32,
    /// Opaque cursor from the previous `next_page_token`: the last
    /// `conversation_id` returned.
    pub page_token:    Option<String>,
}

impl Query for ListSubscriptionsQuery {
    type Response = SubscriptionPage;
}

pub struct ListSubscriptionsHandler<SR> {
    pub subscription_repo: Arc<SR>,
    pub max_page_size:     i32,
}

impl<SR> QueryHandler<ListSubscriptionsQuery> for ListSubscriptionsHandler<SR>
where
    SR: SubscriptionRepository,
{
    type Error = ChatError;

    async fn handle(
        &self,
        envelope: Envelope<ListSubscriptionsQuery>,
    ) -> Result<SubscriptionPage, ChatError> {
        let q = &envelope.payload;

        let subscriber_id = ProfileId::try_from(q.subscriber_id.as_str())?;
        let limit         = q.limit.min(self.max_page_size).max(1);

        let cursor = q
            .page_token
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|t| {
                Uuid::parse_str(t).map_err(|_| ChatError::InvalidPageToken { token: t.to_owned() })
            })
            .transpose()?;

        let (ids, next_cursor) = self
            .subscription_repo
            .list_by_user(&subscriber_id, limit, cursor)
            .await?;

        Ok(SubscriptionPage {
            conversation_ids: ids.into_iter().map(|c| c.as_str()).collect(),
            next_page_token:  next_cursor.map(|id| id.to_string()),
        })
    }
}
