use tonic::{Request, Response, Status};
use uuid::Uuid;

use cqrs::{Envelope, QueryBus};

use crate::application::query::get_following_feed::GetFollowingFeedQuery;

// ── Proto inclusion ───────────────────────────────────────────────────────────

pub mod proto {
    tonic::include_proto!("timeline.v1");
}

pub use proto::timeline_service_server::TimelineServiceServer;

pub struct TimelineServiceHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    query_bus: QB,
}

impl<QB> TimelineServiceHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    pub fn new(query_bus: QB) -> Self {
        Self { query_bus }
    }
}

// ── RPC implementations ───────────────────────────────────────────────────────

impl<QB> TimelineServiceHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    pub async fn get_following_feed(
        &self,
        request: Request<proto::GetFollowingFeedRequest>,
    ) -> Result<Response<proto::GetFollowingFeedResponse>, Status> {
        let req = request.into_inner();

        let query = GetFollowingFeedQuery {
            profile_id: req.profile_id,
            limit:      req.limit,
            page_token: if req.page_token.is_empty() { None } else { Some(req.page_token) },
        };

        let page = self
            .query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), query))
            .await
            .map_err(cqrs_to_status)?;

        let items = page
            .items
            .into_iter()
            .map(|e| proto::FeedItem {
                post_id:         e.post_id.to_string(),
                author_id:       e.author_id.to_string(),
                published_at_ms: e.published_at_ms,
            })
            .collect();

        Ok(Response::new(proto::GetFollowingFeedResponse {
            items,
            next_page_token: page.next_page_token.unwrap_or_default(),
            is_cold:         page.is_cold,
        }))
    }
}

// ── Proto trait implementation ────────────────────────────────────────────────

#[tonic::async_trait]
impl<QB> proto::timeline_service_server::TimelineService for TimelineServiceHandler<QB>
where
    QB: QueryBus + Send + Sync + 'static,
{
    async fn get_following_feed(
        &self,
        request: Request<proto::GetFollowingFeedRequest>,
    ) -> Result<Response<proto::GetFollowingFeedResponse>, Status> {
        self.get_following_feed(request).await
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────────

pub fn cqrs_to_status(err: cqrs::error::CqrsError) -> Status {
    use cqrs::error::CqrsError;
    match err {
        CqrsError::HandlerNotFound { type_name } => {
            Status::unimplemented(format!("no handler registered for {type_name}"))
        }
        CqrsError::DuplicateRegistration { type_name } => {
            Status::internal(format!("duplicate handler for {type_name}"))
        }
        CqrsError::Handler(boxed) => {
            use error::AppError as _;
            let msg       = boxed.to_string();
            let retryable = boxed.is_retryable();
            match boxed.http_status().as_u16() {
                403       => Status::permission_denied(msg),
                404       => Status::not_found(msg),
                409 if retryable => Status::aborted(msg),
                409       => Status::already_exists(msg),
                400 | 422 => Status::failed_precondition(msg),
                503 | 502 => Status::unavailable(msg),
                _         => Status::internal(msg),
            }
        }
    }
}
