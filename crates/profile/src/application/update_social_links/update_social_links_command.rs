// crates/profile/src/application/use_cases/update_social_links/update_social_links_command.rs

use crate::domain::value_objects::{ProfileId, SocialLinks};
use serde::{Deserialize, Serialize};
use tonic::Status;
use shared_kernel::domain::value_objects::RegionCode;
use crate::infrastructure::api::grpc::profile_v1::UpdateSocialLinksRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSocialLinksCommand {
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_links: Option<SocialLinks>,
}

impl UpdateSocialLinksCommand {
    pub fn try_from_proto(req: UpdateSocialLinksRequest, region: RegionCode) -> Result<Self, Status> {
        let profile_id = ProfileId::try_from(req.profile_id)
            .map_err(|e| Status::invalid_argument(format!("ProfileId: {}", e)))?;

        let new_links = req.new_links
            .map(|l| SocialLinks::try_from(l).map_err(|e| Status::invalid_argument(e.to_string())))
            .transpose()?;

        Ok(Self { profile_id, region, new_links })
    }
}