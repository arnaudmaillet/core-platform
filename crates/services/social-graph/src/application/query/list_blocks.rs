use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::SocialGraphRepository;
use crate::domain::entity::BlockEdge;
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

#[derive(Debug, Clone)]
pub struct ListBlocksQuery {
    pub blocker_id: String,
    pub limit:      u32,
    pub page_token: Option<String>,
}

impl Query for ListBlocksQuery {
    type Response = (Vec<BlockEdge>, Option<String>);
}

pub struct ListBlocksHandler {
    repo: Arc<dyn SocialGraphRepository>,
}

impl ListBlocksHandler {
    pub fn new(repo: Arc<dyn SocialGraphRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<ListBlocksQuery> for ListBlocksHandler {
    type Error = SocialGraphError;

    async fn handle(
        &self,
        envelope: Envelope<ListBlocksQuery>,
    ) -> Result<(Vec<BlockEdge>, Option<String>), Self::Error> {
        let q = &envelope.payload;

        let blocker_id = ProfileId::try_from(q.blocker_id.as_str())?;
        let limit      = q.limit.clamp(1, 100) as i32;

        self.repo.list_blocks(&blocker_id, limit, q.page_token.as_deref()).await
    }
}
