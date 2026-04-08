// crates/account/src/application/change_region/change_region_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::ChangeRegionRequest;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRegionCommand {
    pub account_id: AccountId,
    pub new_region: RegionCode,
}

impl ChangeRegionCommand {
    pub fn try_from_proto(req: ChangeRegionRequest) -> Result<Self, tonic::Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid AccountId: {}", e))
            })?,
            new_region: RegionCode::try_from(req.new_region).map_err(|e| {
                tonic::Status::invalid_argument(format!("Invalid RegionCode: {}", e))
            })?,
        })
    }
}
