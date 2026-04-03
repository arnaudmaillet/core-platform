// crates/account/src/application/change_email/change_email_command.rs

use crate::domain::value_objects::Email;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::ChangeEmailRequest;

#[derive(Clone)]
pub struct ChangeEmailCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub new_email: Email,
}

impl ChangeEmailCommand {
    pub fn try_from_proto(req: ChangeEmailRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
            new_email: Email::try_from(req.new_email)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid Email: {}", e)))?,
        })
    }
}