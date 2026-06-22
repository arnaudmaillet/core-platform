use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::{
    application::port::{CommentRepository, CommentSummary},
    domain::value_object::PostId,
    error::CommentError,
};

pub struct ListTopLevelQuery {
    pub post_id:    String,
    pub limit:      i32,
    pub page_token: Option<String>,
}

impl Query for ListTopLevelQuery {
    type Response = (Vec<CommentSummary>, Option<String>);
}

pub struct ListTopLevelHandler<R> {
    pub repository: Arc<R>,
}

impl<R: CommentRepository> QueryHandler<ListTopLevelQuery> for ListTopLevelHandler<R> {
    type Error = CommentError;

    async fn handle(
        &self,
        envelope: Envelope<ListTopLevelQuery>,
    ) -> Result<(Vec<CommentSummary>, Option<String>), CommentError> {
        let q       = &envelope.payload;
        let post_id = PostId::try_from(q.post_id.as_str())?;
        self.repository
            .list_top_level(&post_id, q.limit, q.page_token.as_deref())
            .await
    }
}
