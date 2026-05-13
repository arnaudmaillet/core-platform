// crates/profile/src/application/commands/metadata/update_bio/update_bio_command.rs

use crate::value_objects::{Bio, ProfileId};
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::RegionCode;
use shared_proto::profile::v1::UpdateBioRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateBioCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_bio: Option<Bio>,
}

impl IdentifiableCommand for UpdateBioCommand {
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

impl UpdateBioCommand {
    pub fn try_from_proto(req: UpdateBioRequest) -> Result<Self> {
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

        let new_bio = req
            .new_bio
            .filter(|s| !s.trim().is_empty())
            .map(|s| Bio::try_new(s))
            .transpose()?;

        Ok(Self {
            command_id,
            target,
            new_bio,
        })
    }
}
