use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_personal_service_server::AccountPersonalService as ProtoAccountPersonalService;
use shared_proto::account::v1::{
    ActivateRequest, ActivateResponse, ChangeBirthDateRequest, ChangeBirthDateResponse,
    ChangeEmailRequest, ChangeEmailResponse, ChangePhoneRequest, ChangePhoneResponse,
    DeactivateRequest, DeactivateResponse, UpdateLocaleRequest, UpdateLocaleResponse,
};

use crate::application::context::AccountKernelCtx;
use crate::commands::{
    ActivateCommand, ChangeBirthDateCommand, ChangeEmailCommand, ChangePhoneCommand,
    DeactivateCommand, UpdateLocaleCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

pub struct AccountPersonalService {
    bus: Arc<CommandBus>,
    kernel_ctx: AccountKernelCtx,
}

impl AccountPersonalService {
    pub fn new(bus: Arc<CommandBus>, kernel_ctx: AccountKernelCtx) -> Self {
        Self { bus, kernel_ctx }
    }
}

impl GrpcServiceUtils for AccountPersonalService {
    fn kernel_ctx(&self) -> &AccountKernelCtx {
        &self.kernel_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl ProtoAccountPersonalService for AccountPersonalService {
    async fn change_email(
        &self,
        request: Request<ChangeEmailRequest>,
    ) -> Result<Response<ChangeEmailResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ChangeEmailCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeEmailCommand, (), ChangeEmailResponse>(
            &ctx,
            command,
            ChangeEmailResponse {},
        )
        .await
    }

    async fn change_phone(
        &self,
        request: Request<ChangePhoneRequest>,
    ) -> Result<Response<ChangePhoneResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ChangePhoneCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangePhoneCommand, (), ChangePhoneResponse>(
            &ctx,
            command,
            ChangePhoneResponse {},
        )
        .await
    }

    async fn change_birth_date(
        &self,
        request: Request<ChangeBirthDateRequest>,
    ) -> Result<Response<ChangeBirthDateResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ChangeBirthDateCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeBirthDateCommand, (), ChangeBirthDateResponse>(
            &ctx,
            command,
            ChangeBirthDateResponse {},
        )
        .await
    }

    // async fn change_region(
    //     &self,
    //     request: Request<ChangeRegionRequest>,
    // ) -> Result<Response<ChangeRegionResponse>, Status> {
    //     let (_, extensions, req_inner) = request.into_parts();

    //     let target = req_inner
    //         .target
    //         .as_ref()
    //         .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
    //     let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
    //         Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
    //     })?;

    //     let ctx = self.build_command_context(account_id, &extensions)?;
    //     let command = ChangeRegionCommand::try_from_proto(req_inner)
    //         .map_err(|e| Status::invalid_argument(e.to_string()))?;

    //     self.dispatch_command::<ChangeRegionCommand, (), ChangeRegionResponse>(
    //         &ctx,
    //         command,
    //         ChangeRegionResponse {},
    //     )
    //     .await
    // }

    async fn update_locale(
        &self,
        request: Request<UpdateLocaleRequest>,
    ) -> Result<Response<UpdateLocaleResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = UpdateLocaleCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<UpdateLocaleCommand, (), UpdateLocaleResponse>(
            &ctx,
            command,
            UpdateLocaleResponse {},
        )
        .await
    }

    async fn activate(
        &self,
        request: Request<ActivateRequest>,
    ) -> Result<Response<ActivateResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = ActivateCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ActivateCommand, (), ActivateResponse>(
            &ctx,
            command,
            ActivateResponse {},
        )
        .await
    }

    async fn deactivate(
        &self,
        request: Request<DeactivateRequest>,
    ) -> Result<Response<DeactivateResponse>, Status> {
        let (_, extensions, req) = request.into_parts();

        let target = req
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_ctx(account_id, &extensions)?;
        let command = DeactivateCommand::try_from_proto(req, ctx.region())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<DeactivateCommand, (), DeactivateResponse>(
            &ctx,
            command,
            DeactivateResponse {},
        )
        .await
    }
}
