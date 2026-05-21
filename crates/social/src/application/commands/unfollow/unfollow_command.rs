use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};
use shared_proto::social::v1::UnfollowProfileRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UnfollowCommand {
    pub command_id: Uuid,
    pub follower_id: ProfileId,
    pub target: CommandTarget<ProfileId>,
}

impl IdentifiableCommand for UnfollowCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        format!("{}:{}", self.follower_id, self.target.id)
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }

    fn cache_key(&self) -> Option<String> {
        None
    }
}

impl UnfollowCommand {
    pub fn try_from_proto(req: UnfollowProfileRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target profile metadata"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let follower_id = ProfileId::try_new(req.follower_id)?;

        let target = CommandTarget {
            id: ProfileId::try_new(proto_target.profile_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        Ok(Self {
            command_id,
            follower_id,
            target,
        })
    }
}
