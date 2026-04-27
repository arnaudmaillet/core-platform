// crates/account/src/application/lift_shadowban/lift_shadowban_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_proto::account::v1::LiftShadowbanRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct LiftShadowbanCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: AuditReason,
}

impl LiftShadowbanCommand {
    pub fn try_from_proto(req: LiftShadowbanRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            reason: AuditReason::try_from(req.reason).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
        })
    }
}
