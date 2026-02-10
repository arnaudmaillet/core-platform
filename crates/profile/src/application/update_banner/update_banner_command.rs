use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{RegionCode, Url};
use crate::domain::value_objects::ProfileId;
use crate::infrastructure::api::grpc::profile_v1::UpdateBannerRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBannerCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_banner_url: Url,
}

impl UpdateBannerCommand {
    pub fn try_from_proto(req: UpdateBannerRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            profile_id: ProfileId::try_from(req.profile_id)
                .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?,
            region,
            new_banner_url: Url::try_from(req.new_banner_url)
                .map_err(|e| Status::invalid_argument(format!("BannerUrl: {}", e)))?,
        })
    }
}