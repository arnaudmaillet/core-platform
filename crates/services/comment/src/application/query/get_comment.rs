use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::{
    application::port::CommentRepository,
    domain::{aggregate::Comment, value_object::CommentId},
    error::CommentError,
};

pub struct GetCommentQuery {
    pub comment_id: String,
}

impl Query for GetCommentQuery {
    type Response = Comment;
}

pub struct GetCommentHandler<R> {
    pub repository: Arc<R>,
}

impl<R: CommentRepository> QueryHandler<GetCommentQuery> for GetCommentHandler<R> {
    type Error = CommentError;

    async fn handle(&self, envelope: Envelope<GetCommentQuery>) -> Result<Comment, CommentError> {
        let comment_id = CommentId::try_from(envelope.payload.comment_id.as_str())?;
        self.repository
            .find_by_id(&comment_id)
            .await?
            .ok_or_else(|| CommentError::CommentNotFound { comment_id: comment_id.as_str() })
    }
}
