use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_personal_service_server::AccountPersonalService as ProtoAccountPersonalService;
use shared_proto::account::v1::{
    ActivateRequest, ActivateResponse, ChangeBirthDateRequest, ChangeBirthDateResponse,
    ChangeEmailRequest, ChangeEmailResponse, ChangePhoneNumberRequest, ChangePhoneNumberResponse,
    DeactivateRequest, DeactivateResponse, UpdateLocaleRequest, UpdateLocaleResponse,
};

use crate::application::context::AccountAppContext;
use crate::commands::{
    ActivateCommand, ChangeBirthDateCommand, ChangeEmailCommand, ChangePhoneNumberCommand,
    DeactivateCommand, UpdateLocaleCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;
use shared_kernel::types::AccountId;

pub struct AccountPersonalService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl AccountPersonalService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for AccountPersonalService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
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
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = ChangeEmailCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangeEmailCommand, (), ChangeEmailResponse>(
            &ctx,
            command,
            ChangeEmailResponse {},
        )
        .await
    }

    async fn change_phone_number(
        &self,
        request: Request<ChangePhoneNumberRequest>,
    ) -> Result<Response<ChangePhoneNumberResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = ChangePhoneNumberCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<ChangePhoneNumberCommand, (), ChangePhoneNumberResponse>(
            &ctx,
            command,
            ChangePhoneNumberResponse {},
        )
        .await
    }

    async fn change_birth_date(
        &self,
        request: Request<ChangeBirthDateRequest>,
    ) -> Result<Response<ChangeBirthDateResponse>, Status> {
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = ChangeBirthDateCommand::try_from_proto(req_inner)
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
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = UpdateLocaleCommand::try_from_proto(req_inner)
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
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = ActivateCommand::try_from_proto(req_inner)
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
        let (_, extensions, req_inner) = request.into_parts();

        let target = req_inner
            .target
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing target context"))?;
        let account_id = AccountId::try_from(target.account_id.as_str()).map_err(|e| {
            Status::invalid_argument(format!("Invalid account_id format: {}", e.message))
        })?;

        let ctx = self.build_command_context(account_id, &extensions)?;
        let command = DeactivateCommand::try_from_proto(req_inner)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.dispatch_command::<DeactivateCommand, (), DeactivateResponse>(
            &ctx,
            command,
            DeactivateResponse {},
        )
        .await
    }
}
