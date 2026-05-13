// crates/account/src/application/reactivate_account/command.rs
use serde::Deserialize;
use shared_kernel::{domain::value_objects::AccountId, core::{DomainError, Result}};
use shared_proto::account::v1::ActivateRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ActivateCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
}

impl ActivateCommand {
    pub fn try_from_proto(req: ActivateRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,
        })
    }
}
