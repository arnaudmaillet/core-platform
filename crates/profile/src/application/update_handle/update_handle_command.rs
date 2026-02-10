// crates/profile/src/application/use_cases/update_username/update_username_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::RegionCode;
use crate::domain::value_objects::{Handle, ProfileId};
use crate::infrastructure::api::grpc::profile_v1::UpdateHandleRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateHandleCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_handle: Handle,
}


impl UpdateHandleCommand {
    pub fn try_from_proto(req: UpdateHandleRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            profile_id: ProfileId::try_from(req.profile_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid AccountId: {}", e)))?,
            region,
            new_handle: Handle::try_from(req.new_handle)
                .map_err(|e| Status::invalid_argument(format!("Invalid Handle: {}", e)))?,
        })
    }
}