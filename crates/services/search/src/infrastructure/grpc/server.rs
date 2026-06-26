use tonic::{Request, Response, Status};

use super::handler::{SearchServiceHandler, proto};
use proto::search_service_server::SearchService;

/// Encoded protobuf descriptor set for gRPC server reflection, emitted by
/// `search-api`'s `build.rs`.
pub const FILE_DESCRIPTOR_SET: &[u8] = search_api::FILE_DESCRIPTOR_SET;

#[tonic::async_trait]
impl SearchService for SearchServiceHandler {
    async fn search(
        &self,
        request: Request<proto::SearchRequest>,
    ) -> Result<Response<proto::SearchResponse>, Status> {
        self.search(request).await
    }

    async fn suggest(
        &self,
        request: Request<proto::SuggestRequest>,
    ) -> Result<Response<proto::SuggestResponse>, Status> {
        self.suggest(request).await
    }

    async fn multi_search(
        &self,
        request: Request<proto::MultiSearchRequest>,
    ) -> Result<Response<proto::MultiSearchResponse>, Status> {
        self.multi_search(request).await
    }
}
