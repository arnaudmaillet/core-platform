// crates/profile/src/application/use_cases/update_social_links/update_social_links_command.rs

use crate::domain::value_objects::SocialLinks;
use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use crate::infrastructure::api::grpc::profile_v1::UpdateSocialLinksRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSocialLinksCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_links: Option<SocialLinks>,
}

impl UpdateSocialLinksCommand {
    pub fn try_from_proto(req: UpdateSocialLinksRequest, region: RegionCode) -> Result<Self, Status> {
        let account_id = AccountId::try_from(req.account_id)
            .map_err(|e| Status::invalid_argument(format!("AccountID: {}", e)))?;

        let new_links = req.new_links
            .map(|l| SocialLinks::try_from(l).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        Ok(Self { account_id, region, new_links })
    }
}