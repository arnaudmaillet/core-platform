// crates/account/src/application/ban_account/ban_account_command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_proto::account::v1::BanRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct BanCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: AuditReason,
}

impl BanCommand {
    pub fn try_from_proto(req: BanRequest) -> Result<Self, tonic::Status> {
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
