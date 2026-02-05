// crates/profile/src/application/use_cases/update_username/update_username_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use crate::infrastructure::api::grpc::profile_v1::UpdateUsernameRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUsernameCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_username: Username,
}


impl UpdateUsernameCommand {
    pub fn try_from_proto(req: UpdateUsernameRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid Account ID: {}", e)))?,
            region,
            new_username: Username::try_from(req.new_username)
                .map_err(|e| Status::invalid_argument(format!("Invalid Username: {}", e)))?,
        })
    }
}