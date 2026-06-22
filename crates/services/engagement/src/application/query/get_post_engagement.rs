use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::{PostEngagementSnapshot, ScoreStore};
use crate::domain::value_object::PostId;
use crate::error::EngagementError;

pub struct GetPostEngagementQuery {
    pub post_id: String,
}

impl Query for GetPostEngagementQuery {
    type Response = PostEngagementSnapshot;
}

pub struct GetPostEngagementHandler<S> {
    pub score_store: Arc<S>,
}

impl<S: ScoreStore> QueryHandler<GetPostEngagementQuery> for GetPostEngagementHandler<S> {
    type Error = EngagementError;

    async fn handle(
        &self,
        envelope: Envelope<GetPostEngagementQuery>,
    ) -> Result<PostEngagementSnapshot, EngagementError> {
        let post_id = PostId::try_from(envelope.payload.post_id.as_str())?;

        self.score_store.get_snapshot(&post_id).await
    }
}
