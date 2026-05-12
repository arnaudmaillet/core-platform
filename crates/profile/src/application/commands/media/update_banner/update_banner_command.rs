// crates/profile/src/application/commands/media/update_banner/update_banner_command.rs

use crate::value_objects::ProfileId;
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::domain::value_objects::{RegionCode, Url};
use shared_proto::profile::v1::UpdateBannerRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateBannerCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_banner_url: Url,
}

impl IdentifiableCommand for UpdateBannerCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn profile_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl UpdateBannerCommand {
    pub fn try_from_proto(req: UpdateBannerRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: ProfileId::try_new(proto_target.profile_id)?,
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        Ok(Self {
            command_id,
            target,
            new_banner_url: Url::try_new(req.new_banner_url)?,
        })
    }
}
