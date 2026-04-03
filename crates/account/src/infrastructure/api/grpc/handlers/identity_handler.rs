// crates/account/src/infrastructure/api/grpc/handlers/identity_handler.rs

use shared_kernel::domain::value_objects::RegionCode;
use shared_proto::account::v1::{
    Account,
    ChangeBirthDateRequest,
    ChangeEmailRequest,
    ChangePhoneNumberRequest,
    ChangeRegionRequest,
    DeactivateAccountRequest,
    // RegisterAccountRequest,
    // ResolveIdentityRequest,
    LinkExternalIdentityRequest,
    ReactivateAccountRequest,
    VerifyEmailRequest,
    VerifyPhoneNumberRequest,
    account_identity_service_server::AccountIdentityService,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::application::access_management::link_external_identity::{
    LinkExternalIdentityCommand, LinkExternalIdentityUseCase,
};
use crate::application::access_management::register::{RegisterCommand, RegisterUseCase};
use crate::application::access_management::resolve_identity::{
    ResolveIdentityCommand, ResolveIdentityUseCase,
};
use crate::application::access_management::verify_email::{VerifyEmailCommand, VerifyEmailUseCase};
use crate::application::access_management::verify_phone_number::{
    VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase,
};
use crate::application::lifecycle::activate::{ReactivateUseCase, ActivateCommand};
use crate::application::lifecycle::deactivate::{DeactivateCommand, DeactivateUseCase};
use crate::application::settings::change_birth_date::{
    ChangeBirthDateCommand, ChangeBirthDateUseCase,
};
use crate::application::settings::change_email::{ChangeEmailCommand, ChangeEmailUseCase};
use crate::application::settings::change_phone_number::{
    ChangePhoneNumberCommand, ChangePhoneNumberUseCase,
};
use crate::application::settings::change_region::{ChangeRegionCommand, ChangeRegionUseCase};
use crate::infrastructure::api::grpc::mappers::errors_mapper::ToGrpcStatus;

pub struct IdentityHandler {
    change_email_use_case: Arc<ChangeEmailUseCase>,
    verify_email_use_case: Arc<VerifyEmailUseCase>,
    change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
    verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
    change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
    change_region_use_case: Arc<ChangeRegionUseCase>,
    // register_account_use_case: Arc<RegisterAccountUseCase>,
    resolve_identity_use_case: Arc<ResolveIdentityUseCase>,
    link_external_identity_use_case: Arc<LinkExternalIdentityUseCase>,
    deactivate_account_use_case: Arc<DeactivateUseCase>,
    reactivate_account_use_case: Arc<ReactivateUseCase>,
}

impl IdentityHandler {
    pub fn new(
        change_email_use_case: Arc<ChangeEmailUseCase>,
        verify_email_use_case: Arc<VerifyEmailUseCase>,
        change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
        verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
        change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
        change_region_use_case: Arc<ChangeRegionUseCase>,
        // register_account_use_case: Arc<RegisterAccountUseCase>,
        resolve_identity_use_case: Arc<ResolveIdentityUseCase>,
        link_external_identity_use_case: Arc<LinkExternalIdentityUseCase>,
        deactivate_account_use_case: Arc<DeactivateUseCase>,
        reactivate_account_use_case: Arc<ReactivateUseCase>,
    ) -> Self {
        Self {
            change_email_use_case,
            verify_email_use_case,
            change_phone_number_use_case,
            verify_phone_number_use_case,
            change_birth_date_use_case,
            change_region_use_case,
            // register_account_use_case,
            resolve_identity_use_case,
            link_external_identity_use_case,
            deactivate_account_use_case,
            reactivate_account_use_case,
        }
    }

    fn get_region<T>(&self, request: &Request<T>) -> Result<RegionCode, Status> {
        request
            .extensions()
            .get::<RegionCode>()
            .cloned()
            .ok_or_else(|| Status::internal("Region context missing from metadata"))
    }
}

#[tonic::async_trait]
impl AccountIdentityService for IdentityHandler {
    async fn change_email(
        &self,
        request: Request<ChangeEmailRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeEmailCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .change_email_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = VerifyEmailCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .verify_email_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_phone_number(
        &self,
        request: Request<ChangePhoneNumberRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangePhoneNumberCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .change_phone_number_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn verify_phone_number(
        &self,
        request: Request<VerifyPhoneNumberRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = VerifyPhoneNumberCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .verify_phone_number_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_birth_date(
        &self,
        request: Request<ChangeBirthDateRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeBirthDateCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .change_birth_date_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_region(
        &self,
        request: Request<ChangeRegionRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeRegionCommand::try_from_proto(request.into_inner(), region)?;
        let response = self
            .change_region_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(response.account.into()))
    }

    // async fn register_account(&self, request: Request<RegisterAccountRequest>) -> Result<Response<Account>, Status> {
    //     let region = self.get_region(&request)?;
    //     let command = RegisterAccountCommand::try_from_proto(request.into_inner(), region)?;
    //     let account = self.register_account_use_case.execute(command).await.map_grpc()?;
    //     Ok(Response::new(account.into()))
    // }

    // async fn resolve_identity(&self, request: Request<ResolveIdentityRequest>) -> Result<Response<Account>, Status> {
    //     let region = self.get_region(&request)?;
    //     let command = ResolveIdentityCommand::try_from_proto(request.into_inner())?;
    //     let account = self.resolve_identity_use_case.execute(command).await.map_grpc()?;
    //     Ok(Response::new(account.into()))
    // }

    async fn link_external_identity(
        &self,
        request: Request<LinkExternalIdentityRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = LinkExternalIdentityCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .link_external_identity_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn deactivate_account(
        &self,
        request: Request<DeactivateAccountRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = DeactivateCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .deactivate_account_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn reactivate_account(
        &self,
        request: Request<ReactivateAccountRequest>,
    ) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ActivateCommand::try_from_proto(request.into_inner(), region)?;
        let account = self
            .reactivate_account_use_case
            .execute(command)
            .await
            .map_grpc()?;
        Ok(Response::new(account.into()))
    }
}
