use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::{
    application::port::{PostRepository, PostSummary},
    domain::value_object::ProfileId,
    error::PostError,
};

pub struct ListPostsByProfileQuery {
    pub profile_id: String,
    pub limit:      i32,
    pub page_token: Option<String>,
}

impl Query for ListPostsByProfileQuery {
    type Response = (Vec<PostSummary>, Option<String>);
}

pub struct ListPostsByProfileHandler<R> {
    pub repository: Arc<R>,
}

impl<R: PostRepository> QueryHandler<ListPostsByProfileQuery> for ListPostsByProfileHandler<R> {
    type Error = PostError;

    async fn handle(
        &self,
        envelope: Envelope<ListPostsByProfileQuery>,
    ) -> Result<(Vec<PostSummary>, Option<String>), PostError> {
        let query      = &envelope.payload;
        let profile_id = ProfileId::try_from(query.profile_id.as_str())?;
        self.repository
            .list_by_profile(&profile_id, query.limit, query.page_token.as_deref())
            .await
    }
}
