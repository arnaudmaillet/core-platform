// crates/account/src/application/link_sub_identity/link_sub_identity_command.rs

use shared_kernel::{
    domain::value_objects::{AccountId, SubId},
    errors::{DomainError, Result},
};
use shared_proto::account::v1::LinkSubIdentityRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LinkSubIdentityCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub sub_id: SubId,
}

impl LinkSubIdentityCommand {
    pub fn try_from_proto(req: LinkSubIdentityRequest) -> Result<Self> {
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
            sub_id: SubId::try_from(req.sub_id).map_err(|e| {
                DomainError::Validation {
                    field: "sub_id",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
