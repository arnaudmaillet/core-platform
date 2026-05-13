// crates/profile/src/application/commands/identity/change_handle/change_handle_command.rs

use crate::value_objects::{Handle, ProfileId};
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::RegionCode;
use shared_proto::profile::v1::ChangeHandleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeHandleCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_handle: Handle,
}

impl IdentifiableCommand for ChangeHandleCommand {
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

impl ChangeHandleCommand {
    pub fn try_from_proto(req: ChangeHandleRequest) -> Result<Self> {
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
            new_handle: Handle::try_new(req.new_handle)?,
        })
    }
}
