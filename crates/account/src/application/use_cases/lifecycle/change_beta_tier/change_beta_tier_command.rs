// crates/account/src/application/change_role/change_role_command.rs

use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
};
use shared_proto::account::v1::ChangeBetaTierRequest;
use uuid::Uuid;

use crate::value_objects::BetaTier;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBetaTierCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_tier: BetaTier,
}

impl ChangeBetaTierCommand {
    pub fn try_from_proto(req: ChangeBetaTierRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,
            new_tier: BetaTier::try_from(req.new_tier).map_err(|e| DomainError::Validation {
                field: "new_tier",
                reason: e.to_string(),
            })?,
        })
    }
}
