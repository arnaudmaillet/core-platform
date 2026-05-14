// crates/profile/src/application/commands/media/remove_avatar/remove_avatar_command.rs

use crate::types::ProfileId;
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::RegionCode;
use shared_proto::profile::v1::RemoveAvatarRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct RemoveAvatarCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
}

impl IdentifiableCommand for RemoveAvatarCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
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
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        Ok(Self { command_id, target })
    }
}
