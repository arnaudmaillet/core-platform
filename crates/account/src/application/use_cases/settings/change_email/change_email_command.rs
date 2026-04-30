// crates/account/src/application/change_email/change_email_command.rs

use crate::domain::value_objects::Email;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
};
use shared_proto::account::v1::ChangeEmailRequest;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChangeEmailCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_email: Email,
}

impl ChangeEmailCommand {
    pub fn try_from_proto(req: ChangeEmailRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: AccountId::try_new(&req.account_id).map_err(|e| {
                DomainError::Validation {
                    field: "account_id",
                    reason: e.to_string(),
                }
            })?,
            new_email: Email::try_from(req.new_email).map_err(|e| DomainError::Validation {
                field: "account_id",
                reason: e.to_string(),
            })?,
        })
    }
}
