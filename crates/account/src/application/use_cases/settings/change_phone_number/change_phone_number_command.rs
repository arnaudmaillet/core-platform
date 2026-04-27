// crates/account/src/application/change_email/change_phone_number_command.rs

use crate::domain::value_objects::PhoneNumber;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::ChangePhoneNumberRequest;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChangePhoneNumberCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_phone: PhoneNumber,
}

impl ChangePhoneNumberCommand {
    pub fn try_from_proto(req: ChangePhoneNumberRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            new_phone: PhoneNumber::try_from(req.new_phone).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid PhoneNumber: {}", e))
            })?,
        })
    }
}
