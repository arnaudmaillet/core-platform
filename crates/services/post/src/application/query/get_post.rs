use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::{
    application::port::PostRepository,
    domain::{aggregate::Post, value_object::PostId},
    error::PostError,
};

pub struct GetPostQuery {
    pub post_id: String,
}

impl Query for GetPostQuery {
    type Response = Post;
}

pub struct GetPostHandler<R> {
    pub repository: Arc<R>,
}

impl<R: PostRepository> QueryHandler<GetPostQuery> for GetPostHandler<R> {
    type Error = PostError;

    async fn handle(&self, envelope: Envelope<GetPostQuery>) -> Result<Post, PostError> {
        let query   = &envelope.payload;
        let post_id = PostId::try_from(query.post_id.as_str())?;
        self.repository.find_by_id(&post_id).await?
            .ok_or_else(|| PostError::PostNotFound { post_id: post_id.as_str() })
    }
}
