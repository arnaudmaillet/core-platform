// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_command.rs

use crate::value_objects::{Location, ProfileId};
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::RegionCode;
use shared_proto::profile::v1::UpdateLocationRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateLocationCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_location: Option<Location>,
}

impl IdentifiableCommand for UpdateLocationCommand {
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

impl UpdateLocationCommand {
    pub fn try_from_proto(req: UpdateLocationRequest) -> Result<Self> {
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

        let new_location = req
            .new_location
            .filter(|s| !s.trim().is_empty())
            .map(|s| Location::try_new(s))
            .transpose()?;

        Ok(Self {
            command_id,
            target,
            new_location,
        })
    }
}
