// crates/profile/src/application/use_cases/increment_post_count/increment_post_count_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{PostId, RegionCode};
use crate::domain::value_objects::ProfileId;
use crate::infrastructure::api::grpc::profile_v1::IncrementPostCountRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementPostCountCommand {
    pub profile_id: ProfileId,
    pub post_id: PostId,
    pub region: RegionCode,
}

impl IncrementPostCountCommand {
    pub fn try_from_proto(req: IncrementPostCountRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            profile_id: ProfileId::try_from(req.profile_id)
                .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?,
            region,
            post_id: PostId::try_from(req.post_id)
                .map_err(|e| Status::invalid_argument(format!("PostId: {}", e)))?,
        })
    }
}