use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::{
    application::port::{CommentRepository, CommentSummary},
    domain::value_object::{CommentId, PostId},
    error::CommentError,
};

pub struct ListRepliesQuery {
    pub post_id:    String,
    pub comment_id: String,
    pub limit:      i32,
    pub page_token: Option<String>,
}

impl Query for ListRepliesQuery {
    type Response = (Vec<CommentSummary>, Option<String>);
}

pub struct ListRepliesHandler<R> {
    pub repository: Arc<R>,
}

impl<R: CommentRepository> QueryHandler<ListRepliesQuery> for ListRepliesHandler<R> {
    type Error = CommentError;

    async fn handle(
        &self,
        envelope: Envelope<ListRepliesQuery>,
    ) -> Result<(Vec<CommentSummary>, Option<String>), CommentError> {
        let q          = &envelope.payload;
        let post_id    = PostId::try_from(q.post_id.as_str())?;
        let comment_id = CommentId::try_from(q.comment_id.as_str())?;
        self.repository
            .list_replies(&post_id, &comment_id, q.limit, q.page_token.as_deref())
            .await
    }
}
