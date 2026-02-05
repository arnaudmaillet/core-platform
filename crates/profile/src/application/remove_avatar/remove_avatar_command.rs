use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use crate::infrastructure::api::grpc::profile_v1::RemoveAvatarRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveAvatarCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
}

impl RemoveAvatarCommand {
    pub fn try_from_proto(req: RemoveAvatarRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("AccountID: {}", e)))?,
            region,
        })
    }
}