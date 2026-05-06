// crates/account/src/application/increase_trust_score/command.rs

use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::{AccountId, AuditReason},
    errors::{DomainError, Result},
};
use shared_proto::account::v1::AdjustTrustScoreRequest;
use uuid::Uuid;

use crate::domain::value_objects::TrustDelta;

#[derive(Debug, Deserialize, Clone)]
pub struct IncreaseTrustScoreCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub amount: TrustDelta,
    pub reason: AuditReason,
}

impl IncreaseTrustScoreCommand {
    pub fn try_from_proto(req: AdjustTrustScoreRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,
            amount: TrustDelta::try_from(req.delta).map_err(|e| DomainError::Validation {
                field: "delta",
                reason: e.to_string(),
            })?,
            reason: AuditReason::try_from(req.reason).map_err(|e| DomainError::Validation {
                field: "reason",
                reason: e.to_string(),
            })?,
        })
    }
}
