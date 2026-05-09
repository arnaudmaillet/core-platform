// crates/profile/src/presentation/grpc/profile_service.rs


use shared_kernel::application::LoggingMiddleware;
use shared_proto::profile::v1::{UpdateBioRequest, UpdateDisplayNameRequest};
use tonic::{Request, Response, Status};

use crate::{commands::{UpdateBioCommand, UpdateBioHandler, UpdateDisplayNameCommand, UpdateDisplayNameHandler}, context::ProfileContext};

pub struct ProfileGrpcService {
    ctx: ProfileContext,
}

impl ProfileGrpcService {
    pub fn new(ctx: ProfileContext) -> Self {
        Self { ctx }
    }
}

#[tonic::async_trait]
impl ProfileService for ProfileGrpcService {
    async fn update_display_name(
        &self,
        request: Request<UpdateDisplayNameRequest>,
    ) -> std::result::Result<Response<UpdateDisplayNameResponse>, Status> {
        let req = request.into_inner();

        // 1. Traduction Proto -> Command
        let cmd = UpdateDisplayNameCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // 2. Exécution via le Middleware (Logging + Handler)
        LoggingMiddleware::execute(&UpdateDisplayNameHandler, &self.ctx, cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?; // Idéalement, mapper l'erreur plus finement

        Ok(Response::new(UpdateDisplayNameResponse {}))
    }

    async fn update_bio(
        &self,
        request: Request<UpdateBioRequest>,
    ) -> std::result::Result<Response<UpdateBioResponse>, Status> {
        let req = request.into_inner();

        let cmd = UpdateBioCommand::try_from_proto(req)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        LoggingMiddleware::execute(&UpdateBioHandler, &self.ctx, cmd)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateBioResponse {}))
    }
}
