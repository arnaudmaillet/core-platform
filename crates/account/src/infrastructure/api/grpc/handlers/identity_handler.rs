// crates/account/src/infrastructure/api/grpc/handlers/identity_handler.rs

use shared_kernel::domain::value_objects::RegionCode;
use shared_proto::account::v1::{
    AccountIdentity,
    ActivateRequest,
    ChangeBirthDateRequest,
    ChangeEmailRequest,
    ChangePhoneNumberRequest,
    ChangeRegionRequest,
    DeactivateRequest,
    // ResolveIdentityRequest,
    LinkExternalIdentityRequest,
    RegisterRequest,
    VerifyEmailRequest,
    VerifyPhoneNumberRequest,
    account_identity_service_server::AccountIdentityService,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::application::context::AccountContext;
use crate::application::use_cases::access_management::link_external_identity::{
    LinkExternalIdentityCommand, LinkExternalIdentityUseCase,
};
use crate::application::use_cases::access_management::register::{
    RegisterCommand, RegisterUseCase,
};
use crate::application::use_cases::access_management::resolve_identity::{
    ResolveIdentityCommand, ResolveIdentityUseCase,
};
use crate::application::use_cases::access_management::verify_email::{
    VerifyEmailCommand, VerifyEmailUseCase,
};
use crate::application::use_cases::access_management::verify_phone_number::{
    VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase,
};
use crate::application::use_cases::lifecycle::activate::{ActivateCommand, ActivateUseCase};
use crate::application::use_cases::lifecycle::deactivate::{DeactivateCommand, DeactivateUseCase};
use crate::application::use_cases::settings::change_birth_date::{
    ChangeBirthDateCommand, ChangeBirthDateUseCase,
};
use crate::application::use_cases::settings::change_email::{
    ChangeEmailCommand, ChangeEmailUseCase,
};
use crate::application::use_cases::settings::change_phone_number::{
    ChangePhoneNumberCommand, ChangePhoneNumberUseCase,
};
use crate::application::use_cases::settings::change_region::{
    ChangeRegionCommand, ChangeRegionUseCase,
};
use crate::infrastructure::api::grpc::mappers::errors_mapper::ToGrpcStatus;

pub struct IdentityHandler {
    change_email_use_case: Arc<ChangeEmailUseCase>,
    verify_email_use_case: Arc<VerifyEmailUseCase>,
    change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
    verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
    change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
    change_region_use_case: Arc<ChangeRegionUseCase>,
    register_use_case: Arc<RegisterUseCase>,
    resolve_identity_use_case: Arc<ResolveIdentityUseCase>,
    link_external_identity_use_case: Arc<LinkExternalIdentityUseCase>,
    deactivate_use_case: Arc<DeactivateUseCase>,
    activate_use_case: Arc<ActivateUseCase>,
}

impl IdentityHandler {
    pub fn new(
        change_email_use_case: Arc<ChangeEmailUseCase>,
        verify_email_use_case: Arc<VerifyEmailUseCase>,
        change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
        verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
        change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
        change_region_use_case: Arc<ChangeRegionUseCase>,
        register_use_case: Arc<RegisterUseCase>,
        resolve_identity_use_case: Arc<ResolveIdentityUseCase>,
        link_external_identity_use_case: Arc<LinkExternalIdentityUseCase>,
        deactivate_use_case: Arc<DeactivateUseCase>,
        activate_use_case: Arc<ActivateUseCase>,
    ) -> Self {
        Self {
            change_email_use_case,
            verify_email_use_case,
            change_phone_number_use_case,
            verify_phone_number_use_case,
            change_birth_date_use_case,
            change_region_use_case,
            register_use_case,
            resolve_identity_use_case,
            link_external_identity_use_case,
            deactivate_use_case,
            activate_use_case,
        }
    }

    fn get_ctx<T>(&self, request: &Request<T>) -> Result<AccountContext, Status> {
        request
            .extensions()
            .get::<AccountContext>()
            .cloned()
            .ok_or_else(|| Status::internal("AccountContext missing from extensions"))
    }
}

#[tonic::async_trait]
impl AccountIdentityService for IdentityHandler {
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let region: RegionCode = self.get_region(&request)?;
        let command = RegisterCommand::try_from_proto(request.into_inner(), region)?;
        let res = self.register_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn change_email(
        &self,
        request: Request<ChangeEmailRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = ChangeEmailCommand::try_from_proto(request.into_inner())?;
        let res = self
            .change_email_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = VerifyEmailCommand::try_from_proto(request.into_inner())?;
        let res = self
            .verify_email_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn change_phone_number(
        &self,
        request: Request<ChangePhoneNumberRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = ChangePhoneNumberCommand::try_from_proto(request.into_inner())?;
        let res = self
            .change_phone_number_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn verify_phone_number(
        &self,
        request: Request<VerifyPhoneNumberRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = VerifyPhoneNumberCommand::try_from_proto(request.into_inner())?;
        let res = self
            .verify_phone_number_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn change_birth_date(
        &self,
        request: Request<ChangeBirthDateRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = ChangeBirthDateCommand::try_from_proto(request.into_inner())?;
        let res = self
            .change_birth_date_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn change_region(
        &self,
        request: Request<ChangeRegionRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = ChangeRegionCommand::try_from_proto(request.into_inner())?;
        let res = self
            .change_region_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    // async fn resolve_identity(&self, request: Request<ResolveIdentityRequest>) -> Result<Response<Account>, Status> {
    //     let region = self.get_region(&request)?;
    //     let command = ResolveIdentityCommand::try_from_proto(request.into_inner())?;
    //     let account = self.resolve_identity_use_case.execute(command).await.map_grpc()?;
    //     Ok(Response::new(account.into()))
    // }

    async fn link_external_identity(
        &self,
        request: Request<LinkExternalIdentityRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = LinkExternalIdentityCommand::try_from_proto(request.into_inner())?;
        let res = self
            .link_external_identity_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn deactivate_account(
        &self,
        request: Request<DeactivateRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = DeactivateCommand::try_from_proto(request.into_inner())?;
        let res = self
            .deactivate_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }

    async fn reactivate_account(
        &self,
        request: Request<ActivateRequest>,
    ) -> Result<Response<AccountIdentity>, Status> {
        let ctx = self.get_ctx(&request)?;
        let command = ActivateCommand::try_from_proto(request.into_inner())?;
        let res = self
            .activate_use_case
            .execute(&ctx, command)
            .await
            .map_grpc()?;
        Ok(Response::new(res.into()))
    }
}
