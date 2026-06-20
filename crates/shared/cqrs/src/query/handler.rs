use std::future::Future;

use ::error::AppError;

use crate::envelope::Envelope;

use super::query::Query;

/// Handles a single query type `Q` and returns `Q::Response`.
///
/// Mirrors [`CommandHandler`] on the read side: one registered handler per
/// query type, typed error, fully async.
///
/// ## Example
///
/// ```rust
/// use cqrs::{Query, QueryHandler, Envelope};
///
/// struct GetPostByIdQuery { id: Uuid }
/// impl Query for GetPostByIdQuery {
///     type Response = PostDto;
/// }
///
/// struct GetPostByIdHandler { /* read-db, cache, etc. */ }
///
/// impl QueryHandler<GetPostByIdQuery> for GetPostByIdHandler {
///     type Error = PostServiceError;
///
///     async fn handle(
///         &self,
///         envelope: Envelope<GetPostByIdQuery>,
///     ) -> Result<PostDto, Self::Error> {
///         // read-model lookup here
///         todo!()
///     }
/// }
/// ```
pub trait QueryHandler<Q: Query>: Send + Sync + 'static {
    type Error: AppError;

    fn handle(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, Self::Error>> + Send + '_;
}
