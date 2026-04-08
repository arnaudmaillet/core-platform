use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use shared_proto::account::v1::DeactivateRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateCommand {
    pub account_id: AccountId,
}

impl DeactivateCommand {
    pub fn try_from_proto(proto: DeactivateRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(proto.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
        })
    }
}