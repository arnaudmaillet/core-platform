use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
use crate::infrastructure::api::grpc::profile_v1::UpdateBannerRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBannerCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_banner_url: Url,
}

impl UpdateBannerCommand {
    pub fn try_from_proto(req: UpdateBannerRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("AccountID: {}", e)))?,
            region,
            new_banner_url: Url::try_from(req.new_banner_url)
                .map_err(|e| Status::invalid_argument(format!("BannerURL: {}", e)))?,
        })
    }
}