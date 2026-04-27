// crates/account/src/application/suspend_account/suspend_account_command
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_proto::account::v1::SuspendRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct SuspendCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: AuditReason,
}

impl SuspendCommand {
    pub fn try_from_proto(req: SuspendRequest) -> Result<Self, tonic::Status> {
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
