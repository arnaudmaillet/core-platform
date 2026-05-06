use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_kernel::errors::{DomainError, Result}; // Utilisation de ton Result métier
use shared_proto::account::v1::DeactivateRequest;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: Option<AuditReason>,
}

impl DeactivateCommand {
    pub fn try_from_proto(req: DeactivateRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,

            reason: req
                .reason
                .map(AuditReason::try_new)
                .transpose()
                .map_err(|e| DomainError::Validation {
                    field: "reason",
                    reason: e.to_string(),
                })?,
        })
    }
}
