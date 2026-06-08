// crates/profile/src/application/commands/identity/change_handle/change_handle_command.rs

use crate::types::Handle;
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};
use shared_proto::profile::v1::ChangeHandleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeHandleCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
    pub new_handle: Handle,
}

impl IdentifiableCommand for ChangeHandleCommand {
    type Id = ProfileId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
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
            expected_version: Some(proto_target.expected_version),
        };

        let region = Region::try_new(proto_target.region)?;

        Ok(Self {
            command_id,
            target,
            region,
            new_handle: Handle::try_new(req.new_handle)?,
        })
    }
}
