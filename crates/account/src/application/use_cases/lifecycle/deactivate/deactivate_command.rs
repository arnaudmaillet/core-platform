use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use shared_proto::account::v1::DeactivateRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub reason: Option<AuditReason>,
}

impl DeactivateCommand {
    pub fn try_from_proto(req: DeactivateRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid CommandId: {}", e))
            })?,
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            reason: req
                .reason
                .map(|r| AuditReason::try_from(r))
                .transpose() // Transforme Option<Result<T, E>> en Result<Option<T>, E>
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid Reason: {}", e)))?,
        })
    }
}
