use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_moderation_service_server::AccountModerationService as ProtoAccountModerationService;
use shared_proto::account::v1::{
    BanRequest, BanResponse, ChangeBetaTierRequest, ChangeBetaTierResponse, ChangeRoleRequest,
    ChangeRoleResponse, DecreaseTrustScoreRequest, DecreaseTrustScoreResponse,
    IncreaseTrustScoreRequest, IncreaseTrustScoreResponse, LiftShadowbanRequest,
    LiftShadowbanResponse, ShadowbanRequest, ShadowbanResponse, SuspendRequest, SuspendResponse,
    UnbanRequest, UnbanResponse, UnsuspendRequest, UnsuspendResponse,
};

use crate::application::context::AccountKernelCtx;
use crate::commands::{
    BanCommand, ChangeBetaTierCommand, ChangeRoleCommand, DecreaseTrustScoreCommand,
    IncreaseTrustScoreCommand, LiftShadowbanCommand, ShadowbanCommand, SuspendCommand,
    UnbanCommand, UnsuspendCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

// 🚀 NETTOYAGE : Plus aucun paramètre générique <TM> sur la structure
pub struct AccountModerationService {
    bus: Arc<CommandBus>,
    kernel_ctx: AccountKernelCtx,
}

impl AccountModerationService {
    pub fn new(bus: Arc<CommandBus>, kernel_ctx: AccountKernelCtx) -> Self {
        Self { bus, kernel_ctx }
    }
}

// 🚀 NETTOYAGE : Liaison propre avec le trait utilitaire non-générique
impl GrpcServiceUtils for AccountModerationService {
    fn kernel_ctx(&self) -> &AccountKernelCtx {
        &self.kernel_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoAccountModerationService for AccountModerationService {
    // --- SANCTIONS ---

    async fn ban(&self, request: Request<BanRequest>) -> Result<Response<BanResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = BanCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<BanCommand, (), BanResponse>(&ctx, command, BanResponse {})
            .await
    }

    async fn unban(
        &self,
        request: Request<UnbanRequest>,
    ) -> Result<Response<UnbanResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = UnbanCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UnbanCommand, (), UnbanResponse>(&ctx, command, UnbanResponse {})
            .await
    }

    async fn suspend(
        &self,
        request: Request<SuspendRequest>,
    ) -> Result<Response<SuspendResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = SuspendCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<SuspendCommand, (), SuspendResponse>(
            &ctx,
            command,
            SuspendResponse {},
        )
        .await
    }

    async fn unsuspend(
        &self,
        request: Request<UnsuspendRequest>,
    ) -> Result<Response<UnsuspendResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = UnsuspendCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UnsuspendCommand, (), UnsuspendResponse>(
            &ctx,
            command,
            UnsuspendResponse {},
        )
        .await
    }

    async fn shadowban(
        &self,
        request: Request<ShadowbanRequest>,
    ) -> Result<Response<ShadowbanResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ShadowbanCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ShadowbanCommand, (), ShadowbanResponse>(
            &ctx,
            command,
            ShadowbanResponse {},
        )
        .await
    }

    async fn lift_shadowban(
        &self,
        request: Request<LiftShadowbanRequest>,
    ) -> Result<Response<LiftShadowbanResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = LiftShadowbanCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<LiftShadowbanCommand, (), LiftShadowbanResponse>(
            &ctx,
            command,
            LiftShadowbanResponse {},
        )
        .await
    }

    async fn increase_trust_score(
        &self,
        request: Request<IncreaseTrustScoreRequest>,
    ) -> Result<Response<IncreaseTrustScoreResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = IncreaseTrustScoreCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<IncreaseTrustScoreCommand, (), IncreaseTrustScoreResponse>(
            &ctx,
            command,
            IncreaseTrustScoreResponse {},
        )
        .await
    }

    async fn decrease_trust_score(
        &self,
        request: Request<DecreaseTrustScoreRequest>,
    ) -> Result<Response<DecreaseTrustScoreResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = DecreaseTrustScoreCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<DecreaseTrustScoreCommand, (), DecreaseTrustScoreResponse>(
            &ctx,
            command,
            DecreaseTrustScoreResponse {},
        )
        .await
    }

    async fn change_role(
        &self,
        request: Request<ChangeRoleRequest>,
    ) -> Result<Response<ChangeRoleResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ChangeRoleCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeRoleCommand, (), ChangeRoleResponse>(
            &ctx,
            command,
            ChangeRoleResponse {},
        )
        .await
    }

    async fn change_beta_tier(
        &self,
        request: Request<ChangeBetaTierRequest>,
    ) -> Result<Response<ChangeBetaTierResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str())
            .map_err(|e| Status::invalid_argument(format!("Invalid account_id format: {}", e)))?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ChangeBetaTierCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeBetaTierCommand, (), ChangeBetaTierResponse>(
            &ctx,
            command,
            ChangeBetaTierResponse {},
        )
        .await
    }
}
