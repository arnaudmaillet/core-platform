// crates/account/src/application/link_external_identity/link_external_identity_command.rs

use crate::domain::value_objects::ExternalId;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
};
use shared_proto::account::v1::LinkExternalIdentityRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LinkExternalIdentityCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub external_id: ExternalId,
}

impl LinkExternalIdentityCommand {
    pub fn try_from_proto(req: LinkExternalIdentityRequest) -> Result<Self> {
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
            external_id: ExternalId::try_from(req.external_id).map_err(|e| {
                DomainError::Validation {
                    field: "external_id",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
