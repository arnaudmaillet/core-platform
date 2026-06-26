use tonic::{Request, Response, Status};

use super::handler::{CounterServiceHandler, proto};
use proto::counter_service_server::CounterService;

/// Encoded protobuf descriptor set for gRPC server reflection, emitted by
/// `counter-api`'s `build.rs`.
pub const FILE_DESCRIPTOR_SET: &[u8] = counter_api::FILE_DESCRIPTOR_SET;

#[tonic::async_trait]
impl CounterService for CounterServiceHandler {
    async fn batch_get_counters(
        &self,
        request: Request<proto::BatchGetCountersRequest>,
    ) -> Result<Response<proto::BatchGetCountersResponse>, Status> {
        self.batch_get_counters(request).await
    }

    async fn get_trending(
        &self,
        request: Request<proto::GetTrendingRequest>,
    ) -> Result<Response<proto::GetTrendingResponse>, Status> {
        self.get_trending(request).await
    }

    async fn get_time_series(
        &self,
        request: Request<proto::GetTimeSeriesRequest>,
    ) -> Result<Response<proto::GetTimeSeriesResponse>, Status> {
        self.get_time_series(request).await
    }
}
