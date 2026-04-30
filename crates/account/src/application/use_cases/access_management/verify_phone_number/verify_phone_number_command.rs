// crates/account/src/application/verify_phone_number/command.rs

use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
};
use shared_proto::account::v1::VerifyPhoneNumberRequest;
use uuid::Uuid;

use crate::domain::value_objects::VerificationToken;

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyPhoneNumberCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub token: VerificationToken,
}

impl VerifyPhoneNumberCommand {
    pub fn try_from_proto(req: VerifyPhoneNumberRequest) -> Result<Self> {
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
            token: VerificationToken::try_new(req.token).map_err(|e| DomainError::Validation {
                field: "token",
                reason: e.to_string(),
            })?,
        })
    }
}
