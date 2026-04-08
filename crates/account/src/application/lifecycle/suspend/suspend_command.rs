// crates/account/src/application/suspend_account/suspend_account_command
use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::SuspendRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct SuspendCommand {
    pub account_id: AccountId,
    pub reason: String,
}

impl SuspendCommand {
    pub fn try_from_proto(proto: SuspendRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(proto.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            reason: proto.reason,
        })
    }
}