// crates/account/src/infrastructure/api/grpc/personal_service.rs

use std::sync::Arc;
use tonic::{Request, Response, Status};

use shared_proto::account::v1::account_personal_service_server::AccountPersonalService;
use shared_proto::account::v1::{
    AccountIdentity as ProtoIdentity, ActivateRequest, ChangeBirthDateRequest, ChangeEmailRequest,
    ChangePhoneNumberRequest, ChangeRegionRequest, DeactivateRequest, UpdateLocaleRequest,
    VerifyEmailRequest, VerifyPhoneNumberRequest,
};

use crate::application::context::AccountAppContext;
use crate::application::use_cases::access_management::verify_email::{
    VerifyEmailCommand, VerifyEmailHandler,
};
use crate::application::use_cases::access_management::verify_phone_number::{
    VerifyPhoneNumberCommand, VerifyPhoneNumberHandler,
};
use crate::application::use_cases::lifecycle::activate::{ActivateCommand, ActivateHandler};
use crate::application::use_cases::lifecycle::deactivate::{DeactivateCommand, DeactivateHandler};
use crate::application::use_cases::settings::change_birth_date::{
    ChangeBirthDateCommand, ChangeBirthDateHandler,
};
use crate::application::use_cases::settings::change_email::{
    ChangeEmailCommand, ChangeEmailHandler,
};
use crate::application::use_cases::settings::change_phone_number::{
    ChangePhoneNumberCommand, ChangePhoneNumberHandler,
};
use crate::application::use_cases::settings::change_region::{
    ChangeRegionCommand, ChangeRegionHandler,
};
use crate::application::use_cases::settings::update_locale::{
    UpdateLocaleCommand, UpdateLocaleHandler,
};
use crate::infrastructure::api::grpc::mapper;
use crate::infrastructure::api::grpc::shared::GrpcServiceUtils;
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

        self.execute_and_fetch(
            &ctx,
            command,
            ChangeEmailHandler,
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = VerifyEmailCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch(
            &ctx,
            command,
            VerifyEmailHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            ChangePhoneNumberHandler,
            mapper::map_account_to_identity_proto,
        )
        .await
    }

    async fn verify_phone_number(
        &self,
        request: Request<VerifyPhoneNumberRequest>,
    ) -> Result<Response<ProtoIdentity>, Status> {
        let command = VerifyPhoneNumberCommand::try_from_proto(request.get_ref().clone())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let ctx = self.get_context(&request, &command.account_id).await?;

        self.execute_and_fetch(
            &ctx,
            command,
            VerifyPhoneNumberHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            ChangeBirthDateHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            ChangeRegionHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            UpdateLocaleHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            ActivateHandler,
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

        self.execute_and_fetch(
            &ctx,
            command,
            DeactivateHandler,
            mapper::map_account_to_identity_proto,
        )
        .await
    }
}
