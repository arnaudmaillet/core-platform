// // crates/account/src/infrastructure/api/grpc/handlers/moderation_handler.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};
use shared_proto::account::v1::{
    account_moderation_service_server::AccountModerationService,
    BanAccountRequest, UnbanAccountRequest, ShadowbanRequest,
    LiftShadowbanRequest, IncreaseTrustScoreRequest, DecreaseTrustScoreRequest, SuspendAccountRequest, UnsuspendAccountRequest,
    AccountMetadata as ProtoMetadata
};
use shared_kernel::domain::value_objects::RegionCode;
use crate::application::use_cases::use_cases::suspend_account::{SuspendAccountCommand, SuspendAccountUseCase};
use crate::application::use_cases::use_cases::unsuspend_account::{UnsuspendAccountCommand, UnsuspendAccountUseCase};
use crate::application::use_cases::use_cases::{
    ban_account::*, unban_account::*, shadowban::*,
    lift_shadowban::*, increase_trust_score::*, decrease_trust_score::*
};
use crate::infrastructure::api::grpc::mappers::errors_mapper::ToGrpcStatus;

pub struct ModerationHandler {
    suspend_use_case: Arc<SuspendAccountUseCase>,
    unsuspend_use_case: Arc<UnsuspendAccountUseCase>,
    ban_use_case: Arc<BanAccountUseCase>,
    unban_use_case: Arc<UnbanAccountUseCase>,
    shadowban_use_case: Arc<ShadowbanUseCase>,
    lift_shadowban_use_case: Arc<LiftShadowbanUseCase>,
    increase_trust_use_case: Arc<IncreaseTrustScoreUseCase>,
    decrease_trust_use_case: Arc<DecreaseTrustScoreUseCase>,
}

#[tonic::async_trait]
impl AccountModerationService for ModerationHandler {
    async fn suspend_account(&self, request: Request<SuspendAccountRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = SuspendAccountCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.suspend_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn unsuspend_account(&self, request: Request<UnsuspendAccountRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = UnsuspendAccountCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.unsuspend_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn ban_account(&self, request: Request<BanAccountRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = BanAccountCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.ban_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn unban_account(&self, request: Request<UnbanAccountRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = UnbanAccountCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.unban_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn shadowban(&self, request: Request<ShadowbanRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = ShadowbanCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.shadowban_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn lift_shadowban(&self, request: Request<LiftShadowbanRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = LiftShadowbanCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.lift_shadowban_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn increase_trust_score(&self, request: Request<IncreaseTrustScoreRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = IncreaseTrustScoreCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.increase_trust_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn decrease_trust_score(&self, request: Request<DecreaseTrustScoreRequest>) -> Result<Response<ProtoMetadata>, Status> {
        let region = self.get_region(&request)?;
        let cmd = DecreaseTrustScoreCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.decrease_trust_use_case.execute(cmd).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }
}
