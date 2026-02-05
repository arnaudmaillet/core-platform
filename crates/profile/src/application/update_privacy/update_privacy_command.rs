// crates/profile/src/application/use_cases/update_privacy/update_privacy_command.rs

use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use crate::infrastructure::api::grpc::profile_v1::UpdatePrivacyRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePrivacyCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub is_private: bool,
}

impl UpdatePrivacyCommand {
    pub fn try_from_proto(req: UpdatePrivacyRequest, region: RegionCode) -> Result<Self, Status> {
        Ok(Self {
            account_id: AccountId::try_from(req.account_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid Account ID: {}", e)))?,
            region,
            is_private: req.is_private,
        })
    }
}