// crates/account/src/application/change_email/change_phone_number_command.rs

use shared_kernel::{
    domain::value_objects::{AccountId, PhoneNumber},
    core::{DomainError, Result},
};
use shared_proto::account::v1::ChangePhoneNumberRequest;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChangePhoneNumberCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_phone: PhoneNumber,
}

impl ChangePhoneNumberCommand {
    pub fn try_from_proto(req: ChangePhoneNumberRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,
            new_phone: PhoneNumber::try_from(req.new_phone).map_err(|e| {
                DomainError::Validation {
                    field: "new_phone",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
