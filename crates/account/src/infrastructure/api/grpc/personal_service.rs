// crates/account/src/infrastructure/api/grpc/personal_service.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_personal_service_server::AccountPersonalService;
use shared_proto::account::v1::{
    AccountIdentity as ProtoIdentity, ActivateRequest, ChangeBirthDateRequest, ChangeEmailRequest,
    ChangePhoneNumberRequest, ChangeRegionRequest, DeactivateRequest, UpdateLocaleRequest,
};

use crate::application::context::AccountAppContext;
use crate::infrastructure::api::grpc::mapper;
use crate::infrastructure::api::grpc::shared::GrpcServiceUtils;
use crate::use_cases::{
    ActivateCommand, ChangeBirthDateCommand, ChangeEmailCommand, ChangePhoneNumberCommand,
    ChangeRegionCommand, DeactivateCommand, UpdateLocaleCommand,
};
use shared_kernel::application::CommandBus;

pub struct GrpcPersonalService {
    bus: Arc<CommandBus>,
    app_ctx: Arc<AccountAppContext>,
}

impl GrpcPersonalService {
    pub fn new(bus: Arc<CommandBus>, app_ctx: Arc<AccountAppContext>) -> Self {
        Self { bus, app_ctx }
    }
}

impl GrpcServiceUtils for GrpcPersonalService {
    fn app_ctx(&self) -> &AccountAppContext {
        &self.app_ctx
    }
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
}

#[tonic::async_trait]
impl AccountPersonalService for GrpcPersonalService {
    async fn change_email(
        &self,
        request: Request<ChangeEmailRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = ChangeEmailCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        // On spécifie () comme type de retour attendu du bus
        self.execute_and_fetch::<ChangeEmailCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn change_phone_number(
        &self,
        request: Request<ChangePhoneNumberRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = ChangePhoneNumberCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ChangePhoneNumberCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn change_birth_date(
        &self,
        request: Request<ChangeBirthDateRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = ChangeBirthDateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ChangeBirthDateCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn change_region(
        &self,
        request: Request<ChangeRegionRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = ChangeRegionCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ChangeRegionCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn update_locale(
        &self,
        request: Request<UpdateLocaleRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = UpdateLocaleCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<UpdateLocaleCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn activate(
        &self,
        request: Request<ActivateRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = ActivateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<ActivateCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn deactivate(
        &self,
        request: Request<DeactivateRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = DeactivateCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch::<DeactivateCommand, (), ProtoIdentity, _>(
            &ctx,
            command,
            (),
            mapper::map_account_to_identity_proto,
        )
        .await
    }
}
