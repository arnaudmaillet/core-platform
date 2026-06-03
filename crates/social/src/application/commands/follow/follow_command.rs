use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};
use shared_proto::social::v1::FollowProfileRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct FollowCommand {
    pub command_id: Uuid,
    pub follower_id: ProfileId,
    pub target: CommandTarget<ProfileId>,
}

impl IdentifiableCommand for FollowCommand {
    type Id = ProfileId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn cache_enabled(&self) -> bool {
        false
    }
}

impl FollowCommand {
    pub fn try_from_proto(req: FollowProfileRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target profile metadata"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let follower_id = ProfileId::try_new(req.follower_id)?;

        let target = CommandTarget {
            id: ProfileId::try_new(proto_target.profile_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self {
            command_id,
            follower_id,
            target,
        })
    }
}
