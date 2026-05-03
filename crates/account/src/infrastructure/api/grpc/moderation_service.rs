// crates/account/src/infrastructure/api/grpc/moderation_service.rs

use shared_proto::account::v1::{
    AccountGovernance as ProtoGovernance, AdjustTrustScoreRequest, ChangeRoleRequest,
    ModerationRequest,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_moderation_service_server::AccountModerationService;

use crate::application::context::AccountAppContext;
use crate::infrastructure::api::grpc::mapper;
use crate::infrastructure::api::grpc::shared::GrpcServiceUtils;
use crate::use_cases::{
    BanCommand, ChangeRoleCommand, DecreaseTrustScoreCommand, IncreaseTrustScoreCommand,
    LiftShadowbanCommand, ShadowbanCommand, SuspendCommand, UnbanCommand, UnsuspendCommand,
};
use shared_kernel::application::CommandBus;

pub struct GrpcModerationService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl GrpcModerationService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for GrpcModerationService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl AccountModerationService for GrpcModerationService {
    async fn ban(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = BanCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        // On spécifie () comme Output car Ban renvoie Result<()>
        self.execute_and_fetch::<BanCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn unban(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = UnbanCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<UnbanCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn suspend(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = SuspendCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<SuspendCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn unsuspend(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = UnsuspendCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<UnsuspendCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn shadowban(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = ShadowbanCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ShadowbanCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn lift_shadowban(
        &self,
        request: Request<ModerationRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = LiftShadowbanCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<LiftShadowbanCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }

    async fn adjust_trust_score(
        &self,
        request: Request<AdjustTrustScoreRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let req_ref = request.get_ref();

        if req_ref.delta >= 0 {
            let command = IncreaseTrustScoreCommand::try_from_proto(req_ref.clone())
                .map_err(|e| Status::invalid_argument(e.to_string()))?;
            let ctx = self.get_context(&request, &command.account_id).await?;

            self.execute_and_fetch::<IncreaseTrustScoreCommand, (), ProtoGovernance, _>(
                &ctx,
                command,
                (),
                mapper::map_account_to_governance_proto,
            )
            .await
        } else {
            let command = DecreaseTrustScoreCommand::try_from_proto(req_ref.clone())
                .map_err(|e| Status::invalid_argument(e.to_string()))?;
            let ctx = self.get_context(&request, &command.account_id).await?;

            self.execute_and_fetch::<DecreaseTrustScoreCommand, (), ProtoGovernance, _>(
                &ctx,
                command,
                (),
                mapper::map_account_to_governance_proto,
            )
            .await
        }
    }

    async fn change_role(
        &self,
        request: Request<ChangeRoleRequest>,
    ) -> Result<Response<ProtoGovernance>, Status> {
        let command = ChangeRoleCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ChangeRoleCommand, (), ProtoGovernance, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_governance_proto,
        )
        .await
    }
}
