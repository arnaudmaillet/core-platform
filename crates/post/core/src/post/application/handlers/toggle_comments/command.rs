// crates/post/src/application/commands/toggle_comments.rs

use post_proto_bridge::v1::ToggleCommentsRequest;
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, Region};
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ToggleCommentsCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<PostId>,
    pub region: Region,
    pub allowed: bool,
}

impl IdentifiableCommand for ToggleCommentsCommand {
    type Id = PostId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<PostId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }
}

impl ToggleCommentsCommand {
    pub fn try_from_proto(req: ToggleCommentsRequest, region: Region) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target"))?;

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|_| Error::validation("command_id", "Invalid UUID"))?,
            target: CommandTarget {
                id: PostId::try_from(proto_target.post_id)?,
                expected_version: None,
            },
            region,
            allowed: req.allowed,
        })
    }
}
