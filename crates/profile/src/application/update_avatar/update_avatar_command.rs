use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url};
use crate::infrastructure::api::grpc::profile_v1::UpdateAvatarRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAvatarCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_avatar_url: Url,
}

impl UpdateAvatarCommand {
    pub fn try_from_proto(req: UpdateAvatarRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("AccountID: {}", e)))?,
            region,
            new_avatar_url: Url::try_from(req.new_avatar_url)
                .map_err(|e| Status::invalid_argument(format!("AvatarURL: {}", e)))?,
        })
    }
}