// crates/account/src/application/unsuspend_account/unsuspend_account_command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::UnsuspendRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct UnsuspendCommand {
    pub account_id: AccountId,
}

impl UnsuspendCommand {
    pub fn try_from_proto(proto: UnsuspendRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(proto.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
        })
    }
}