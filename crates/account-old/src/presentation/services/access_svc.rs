use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_kernel::command::CommandBus;
use shared_kernel::types::{AccountId, SubId};

use shared_proto::account::v1::account_access_service_server::AccountAccessService as ProtoAccountAccessService;
use shared_proto::account::v1::{
    LinkSubIdentityRequest, LinkSubIdentityResponse, ResolveIdentityRequest,
    ResolveIdentityResponse, VerifyEmailRequest, VerifyEmailResponse, VerifyPhoneRequest,
    VerifyPhoneResponse,
};

use crate::application::context::AccountKernelCtx;
use crate::commands::{LinkSubIdentityCommand, VerifyEmailCommand, VerifyPhoneCommand};
use crate::presentation::utils::GrpcServiceUtils;

pub struct AccountAccessService {
    bus: Arc<CommandBus>,
    kernel_ctx: AccountKernelCtx,
}

impl AccountAccessService {
    pub fn new(bus: Arc<CommandBus>, kernel_ctx: AccountKernelCtx) -> Self {
        Self { bus, kernel_ctx }
    }
}

impl GrpcServiceUtils for AccountAccessService {
    fn kernel_ctx(&self) -> &AccountKernelCtx {
        &self.kernel_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoAccountAccessService for AccountAccessService {
    async fn link_sub_identity(
        &self,
        request: Request<LinkSubIdentityRequest>,
    ) -> Result<Response<LinkSubIdentityResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = LinkSubIdentityCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<LinkSubIdentityCommand, (), LinkSubIdentityResponse>(
            &ctx,
            command,
            LinkSubIdentityResponse {},
        )
        .await
    }

    async fn resolve_identity(
        &self,
        request: Request<ResolveIdentityRequest>,
    ) -> Result<Response<ResolveIdentityResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let sub_id =
            SubId::try_new(req.sub_id).map_err(|e| Status::invalid_argument(e.to_string()))?;

        let query_ctx = self.build_query_ctx(&extensions)?;

        let account = self
            .kernel_ctx
            .account_repo()
            .find_by_sub_id(query_ctx.region(), &sub_id, None)
            .await
            .map_err(|e| {
                Status::internal(format!("Database error during identity resolution: {}", e))
            })?
            .ok_or_else(|| {
                Status::not_found(
                    "No account associated with this sub identity in the target region",
                )
            })?;

        Ok(Response::new(ResolveIdentityResponse {
            account_id: account.account_id().to_string(),
            state: account.identity().state().to_string(),
            role: account.governance().role().to_string(),
        }))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<VerifyEmailResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let proto_target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target identity context"))?;

        let account_id = AccountId::try_from(proto_target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = VerifyEmailCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<VerifyEmailCommand, (), VerifyEmailResponse>(
            &ctx,
            command,
            VerifyEmailResponse {},
        )
        .await
    }

    async fn verify_phone(
        &self,
        request: Request<VerifyPhoneRequest>,
    ) -> Result<Response<VerifyPhoneResponse>, Status> {
        let (_, extensions, req) = request.into_parts();
        let proto_target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target identity context"))?;

        let account_id = AccountId::try_from(proto_target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = VerifyPhoneCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<VerifyPhoneCommand, (), VerifyPhoneResponse>(
            &ctx,
            command,
            VerifyPhoneResponse {},
        )
        .await
    }
}
