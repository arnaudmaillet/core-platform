// crates/account/src/application/change_email/change_phone_number_command.rs

use crate::domain::value_objects::PhoneNumber;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::ChangePhoneNumberRequest;

#[derive(Clone)]
pub struct ChangePhoneNumberCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub new_phone: PhoneNumber,
}

impl ChangePhoneNumberCommand {
    pub fn try_from_proto(req: ChangePhoneNumberRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
            new_phone: PhoneNumber::try_from(req.new_phone)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid PhoneNumber: {}", e)))?,
        })
    }
}