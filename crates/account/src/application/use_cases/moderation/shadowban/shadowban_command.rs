// crates/account/src/application/shadowban_account/shadowban_command.rs

use shared_kernel::{domain::value_objects::{AccountId, AuditReason}, errors::{DomainError, Result}};
use shared_proto::account::v1::ModerationRequest;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ShadowbanCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: AuditReason,
}

impl ShadowbanCommand {
    pub fn try_from_proto(req: ModerationRequest) -> Result<Self> {
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

            reason: AuditReason::try_from(req.reason).map_err(|e| DomainError::Validation {
                field: "reason",
                reason: e.to_string(),
            })?,
        })
    }
}
