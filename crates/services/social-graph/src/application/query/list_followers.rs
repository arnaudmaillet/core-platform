use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::SocialGraphRepository;
use crate::domain::entity::FollowEdge;
use crate::domain::value_object::ProfileId;
use crate::error::SocialGraphError;

#[derive(Debug, Clone)]
pub struct ListFollowersQuery {
    pub followee_id: String,
    pub limit:       u32,
    pub page_token:  Option<String>,
}

impl Query for ListFollowersQuery {
    type Response = (Vec<FollowEdge>, Option<String>);
}

pub struct ListFollowersHandler {
    repo: Arc<dyn SocialGraphRepository>,
}

impl ListFollowersHandler {
    pub fn new(repo: Arc<dyn SocialGraphRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<ListFollowersQuery> for ListFollowersHandler {
    type Error = SocialGraphError;

    async fn handle(
        &self,
        envelope: Envelope<ListFollowersQuery>,
    ) -> Result<(Vec<FollowEdge>, Option<String>), Self::Error> {
        let q = &envelope.payload;

        let followee_id = ProfileId::try_from(q.followee_id.as_str())?;
        let limit       = q.limit.clamp(1, 100) as i32;

        self.repo.list_followers(&followee_id, limit, q.page_token.as_deref()).await
    }
}
