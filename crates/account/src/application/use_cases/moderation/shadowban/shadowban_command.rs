// crates/account/src/application/shadowban_account/shadowban_command.rs

use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_proto::account::v1::ShadowbanRequest;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ShadowbanCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: AuditReason,
}

impl ShadowbanCommand {
    pub fn try_from_proto(req: ShadowbanRequest) -> Result<Self, tonic::Status> {
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
