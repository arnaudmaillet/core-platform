// crates/account/src/infrastructure/api/grpc/handlers/identity_handler.rs

use std::sync::Arc;
use shared_proto::account::v1::{
    account_identity_service_server::AccountIdentityService,
    ChangeEmailRequest, 
    VerifyEmailRequest, 
    ChangePhoneNumberRequest, 
    VerifyPhoneNumberRequest, 
    ChangeBirthDateRequest, 
    ChangeRegionRequest, 
    Account,
};
use tonic::{Request, Response, Status};
use shared_kernel::domain::value_objects::RegionCode;

use crate::application::use_cases::change_birth_date::{ChangeBirthDateCommand, ChangeBirthDateUseCase};
use crate::application::use_cases::change_email::{ChangeEmailCommand, ChangeEmailUseCase};
use crate::application::use_cases::change_phone_number::{ChangePhoneNumberCommand, ChangePhoneNumberUseCase};
use crate::application::use_cases::change_region::{ChangeRegionCommand, ChangeRegionUseCase};
use crate::application::use_cases::verify_email::{VerifyEmailCommand, VerifyEmailUseCase};
use crate::application::use_cases::verify_phone_number::{VerifyPhoneNumberCommand, VerifyPhoneNumberUseCase};
use crate::infrastructure::api::grpc::mappers::errors_mapper::ToGrpcStatus;

pub struct IdentityHandler {
    change_email_use_case: Arc<ChangeEmailUseCase>,
    verify_email_use_case: Arc<VerifyEmailUseCase>,
    change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
    verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
    change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
    change_region_use_case: Arc<ChangeRegionUseCase>,
}

impl IdentityHandler {
    pub fn new(
        change_email_use_case: Arc<ChangeEmailUseCase>,
        verify_email_use_case: Arc<VerifyEmailUseCase>,
        change_phone_number_use_case: Arc<ChangePhoneNumberUseCase>,
        verify_phone_number_use_case: Arc<VerifyPhoneNumberUseCase>,
        change_birth_date_use_case: Arc<ChangeBirthDateUseCase>,
        change_region_use_case: Arc<ChangeRegionUseCase>,
    ) -> Self {
        Self {
            change_email_use_case,
            verify_email_use_case,
            change_phone_number_use_case,
            verify_phone_number_use_case,
            change_birth_date_use_case,
            change_region_use_case,
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
    async fn change_email(&self, request: Request<ChangeEmailRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeEmailCommand::try_from_proto(request.into_inner(), region)?;
        let account = self.change_email_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn verify_email(&self, request: Request<VerifyEmailRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = VerifyEmailCommand::try_from_proto(request.into_inner(), region)?;
        let account = self.verify_email_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_phone_number(&self, request: Request<ChangePhoneNumberRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangePhoneNumberCommand::try_from_proto(request.into_inner(), region)?;
        let account = self.change_phone_number_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn verify_phone_number(&self, request: Request<VerifyPhoneNumberRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = VerifyPhoneNumberCommand::try_from_proto(request.into_inner(), region)?;
        let account = self.verify_phone_number_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_birth_date(&self, request: Request<ChangeBirthDateRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeBirthDateCommand::try_from_proto(request.into_inner(), region)?;
        let account = self.change_birth_date_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(account.into()))
    }

    async fn change_region(&self, request: Request<ChangeRegionRequest>) -> Result<Response<Account>, Status> {
        let region = self.get_region(&request)?;
        let command = ChangeRegionCommand::try_from_proto(request.into_inner(), region)?;
        let response = self.change_region_use_case.execute(command).await.map_grpc()?;
        Ok(Response::new(response.account.into()))
    }
}