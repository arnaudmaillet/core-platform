// crates/post/src/application/commands/update_visibility.rs

use crate::domain::types::VisibilityLevel;
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, Region};
use shared_proto::post::v1::ChangeVisibilityRequest;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeVisibilityCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<PostId>,
    pub new_visibility: VisibilityLevel,
}

impl IdentifiableCommand for ChangeVisibilityCommand {
    type Id = PostId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<PostId> {
        &self.target
    }
}

impl ChangeVisibilityCommand {
    pub fn try_from_proto(req: ChangeVisibilityRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target"))?;

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|_| Error::validation("command_id", "Invalid UUID"))?,
            target: CommandTarget {
                id: PostId::try_from(proto_target.post_id)?,
                region: Region::try_new(proto_target.region)?,
                expected_version: None,
            },
            new_visibility: VisibilityLevel::from_str(&req.new_visibility_level)?,
        })
    }
}
