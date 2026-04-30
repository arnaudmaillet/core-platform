// crates/account/src/application/add_push_token/add_push_token_command.rs
use shared_kernel::domain::value_objects::{AccountId, PushToken};
use shared_kernel::errors::{DomainError, Result};
use shared_proto::account::v1::AddPushTokenRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AddPushTokenCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub token: PushToken,
}

impl AddPushTokenCommand {
    pub fn try_from_proto(req: AddPushTokenRequest) -> Result<Self> {
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

            token: PushToken::try_new(&req.token).map_err(|e| {
                DomainError::Validation {
                    field: "push_token",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
