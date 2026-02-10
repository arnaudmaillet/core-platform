// crates/profile/src/application/use_cases/update_location/update_location_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{LocationLabel, RegionCode};
use crate::domain::value_objects::ProfileId;
use crate::infrastructure::api::grpc::profile_v1::UpdateLocationLabelRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLocationLabelCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_location: Option<LocationLabel>,
}


impl UpdateLocationLabelCommand {
    pub fn try_from_proto(req: UpdateLocationLabelRequest, region: RegionCode) -> Result<Self, Status> {
        let profile_id = ProfileId::try_from(req.profile_id)
            .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?;

        let new_location = req.new_location_label
            .filter(|s| !s.trim().is_empty())
            .map(|s| LocationLabel::try_from(s).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        Ok(Self { profile_id, region, new_location })
    }
}