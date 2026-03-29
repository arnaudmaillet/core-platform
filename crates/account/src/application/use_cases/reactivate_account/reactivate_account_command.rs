// crates/account/src/application/reactivate_account/command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::ReactivateAccountRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct ReactivateAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}

impl ReactivateAccountCommand {
    pub fn try_from_proto(req: ReactivateAccountRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.id)
                .map_err(|e| tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region_code: region,
        })
    }
}