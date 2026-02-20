use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::RegionCode;
use crate::domain::value_objects::ProfileId;
use crate::infrastructure::api::grpc::profile_v1::RemoveAvatarRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveAvatarCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
}

impl RemoveAvatarCommand {
    pub fn try_from_proto(req: RemoveAvatarRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            profile_id: ProfileId::try_from(req.profile_id)
                .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?,
            region,
        })
    }
}