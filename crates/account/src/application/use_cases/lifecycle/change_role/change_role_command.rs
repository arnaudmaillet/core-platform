// crates/account/src/application/change_role/change_role_command.rs

use crate::domain::value_objects::AccountRole;
use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::{AccountId, AuditReason},
    core::{DomainError, Result},
};
use shared_proto::account::v1::ChangeRoleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRoleCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_role: AccountRole,
    pub reason: AuditReason,
}

impl ChangeRoleCommand {
    pub fn try_from_proto(req: ChangeRoleRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,
            new_role: AccountRole::try_from(req.new_role).map_err(|e| DomainError::Validation {
                field: "new_role",
                reason: e.to_string(),
            })?,
            reason: AuditReason::try_from(req.reason).map_err(|e| DomainError::Validation {
                field: "reason",
                reason: e.to_string(),
            })?,
        })
    }
}
