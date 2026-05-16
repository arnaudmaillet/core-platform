// crates/account/src/infrastructure/api/grpc/personal_service.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_personal_service_server::AccountPersonalService as ProtoAccountPersonalService;
use shared_proto::account::v1::{
    ActivateRequest, ActivateResponse, ChangeBirthDateRequest, ChangeBirthDateResponse,
    ChangeEmailRequest, ChangeEmailResponse, ChangePhoneNumberRequest, ChangePhoneNumberResponse,
    ChangeRegionRequest, ChangeRegionResponse, DeactivateRequest, DeactivateResponse,
    UpdateLocaleRequest, UpdateLocaleResponse,
};

use crate::application::context::AccountAppContext;
use crate::commands::{
    ActivateCommand, ChangeBirthDateCommand, ChangeEmailCommand, ChangePhoneNumberCommand,
    ChangeRegionCommand, DeactivateCommand, UpdateLocaleCommand,
};
use crate::presentation::utils::GrpcServiceUtils;
use shared_kernel::command::CommandBus;

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
    // --- INFORMATIONS PERSONNELLES ---

    async fn change_email(
        &self,
        request: Request<ChangeEmailRequest>,
    ) -> Result<Response<ChangeEmailResponse>, Status> {
        let command = ChangeEmailCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

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
        let command = ChangePhoneNumberCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

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
        let command = ChangeBirthDateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

        self.dispatch_command::<ChangeBirthDateCommand, (), ChangeBirthDateResponse>(
            &ctx,
            command,
            ChangeBirthDateResponse {},
        )
        .await
    }

    // --- LOCALISATION ---

    async fn change_region(
        &self,
        request: Request<ChangeRegionRequest>,
    ) -> Result<Response<ChangeRegionResponse>, Status> {
        let command = ChangeRegionCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

        self.dispatch_command::<ChangeRegionCommand, (), ChangeRegionResponse>(
            &ctx,
            command,
            ChangeRegionResponse {},
        )
        .await
    }

    async fn update_locale(
        &self,
        request: Request<UpdateLocaleRequest>,
    ) -> Result<Response<UpdateLocaleResponse>, Status> {
        let command = UpdateLocaleCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

        self.dispatch_command::<UpdateLocaleCommand, (), UpdateLocaleResponse>(
            &ctx,
            command,
            UpdateLocaleResponse {},
        )
        .await
    }

    // --- CYCLE DE VIE UTILISATEUR ---

    async fn activate(
        &self,
        request: Request<ActivateRequest>,
    ) -> Result<Response<ActivateResponse>, Status> {
        let command = ActivateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

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
        let command = DeactivateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.target.id)?;

        self.dispatch_command::<DeactivateCommand, (), DeactivateResponse>(
            &ctx,
            command,
            DeactivateResponse {},
        )
        .await
    }
}
