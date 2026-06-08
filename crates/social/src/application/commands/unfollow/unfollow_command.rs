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
    pub region: Region,
}

impl IdentifiableCommand for UnfollowCommand {
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

    fn resolve_cache_key(&self) -> Option<String> {
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
            expected_version: Some(proto_target.expected_version),
        };

        let region = Region::try_new(proto_target.region)?;

        Ok(Self {
            command_id,
            follower_id,
            target,
            region,
        })
    }
}
