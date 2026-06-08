// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_command.rs
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::{ProfileId, Region};
use shared_proto::profile::v1::RemoveAvatarRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct RemoveAvatarCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
}

impl IdentifiableCommand for RemoveAvatarCommand {
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

impl RemoveAvatarCommand {
    pub fn try_from_proto(req: RemoveAvatarRequest) -> Result<Self> {
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
            region,
            target,
        })
    }
}
