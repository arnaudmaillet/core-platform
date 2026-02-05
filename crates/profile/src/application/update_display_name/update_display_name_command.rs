// crates/profile/src/application/use_cases/update_display_name/update_display_name_command.rs

use crate::domain::value_objects::DisplayName;
use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use crate::infrastructure::api::grpc::profile_v1::UpdateDisplayNameRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDisplayNameCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_display_name: DisplayName,
}


impl UpdateDisplayNameCommand {
    pub fn try_from_proto(req: UpdateDisplayNameRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid Account ID: {}", e)))?,
            region,
            new_display_name: DisplayName::try_from(req.new_display_name)
                .map_err(|e| Status::invalid_argument(format!("Invalid Display Name: {}", e)))?,
        })
    }
}