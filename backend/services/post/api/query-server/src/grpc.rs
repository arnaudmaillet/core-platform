use crate::GrpcPostQueryMapper;
use crate::map_domain_err_to_status;
use post_assembly::PostQueryContainer;
use post_proto_bridge::v1::post_query_service_server::PostQueryService as ProtoPostQueryService;
use post_proto_bridge::v1::*;

use shared_kernel::core::PageQuery;
use shared_kernel::types::{PostId, ProfileId};
use tonic::{Request, Response, Status};

pub struct PostQueryService {
    container: PostQueryContainer,
}

impl PostQueryService {
    pub fn new(container: PostQueryContainer) -> Self {
        Self { container }
    }
}

#[tonic::async_trait]
impl ProtoPostQueryService for PostQueryService {
    async fn get_post(
        &self,
        request: Request<GetPostRequest>,
    ) -> Result<Response<PostDetails>, Status> {
        let (_, _, req) = request.into_parts();
        let post_id =
            PostId::try_from(req.post_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let post_details = self
            .container
            .query_service
            .get_post_detail(&post_id)
            .await
            .map_err(map_domain_err_to_status)?
            .ok_or_else(|| Status::not_found("Post not found"))?;

        let proto_details = GrpcPostQueryMapper::to_proto_details(&post_details);
        Ok(Response::new(proto_details))
    }

    async fn get_posts_by_author(
        &self,
        request: Request<GetPostsByAuthorRequest>,
    ) -> Result<Response<GetPostsByAuthorResponse>, Status> {
        // Remplacement de _meta et _ext par _ ici aussi
        let (_, _, req) = request.into_parts();

        let author_id = ProfileId::try_new(req.author_id.clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let query = PageQuery {
            limit: req.limit as usize,
            cursor: req.cursor,
        };

        let paged_details = self
            .container
            .query_service
            .get_posts_by_author_details(&author_id, query)
            .await
            .map_err(map_domain_err_to_status)?;

        Ok(Response::new(GetPostsByAuthorResponse {
            items: paged_details
                .items
                .iter()
                .map(GrpcPostQueryMapper::to_proto_details)
                .collect(),
            next_cursor: paged_details.next_cursor,
        }))
    }
}
