// crates/profile/src/application/use_cases/update_privacy/update_privacy_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::RegionCode;
use crate::domain::value_objects::ProfileId;
use crate::infrastructure::api::grpc::profile_v1::UpdatePrivacyRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePrivacyCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub is_private: bool,
}

impl UpdatePrivacyCommand {
    pub fn try_from_proto(req: UpdatePrivacyRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            profile_id: ProfileId::try_from(req.profile_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid ProfileId: {}", e)))?,
            region,
            is_private: req.is_private,
        })
    }
}