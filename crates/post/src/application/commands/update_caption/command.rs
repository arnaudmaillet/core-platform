// crates/post/src/application/commands/update_caption.rs

use crate::domain::types::Caption;
use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, Region};
use shared_proto::post::v1::UpdateCaptionRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateCaptionCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<PostId>,
    pub new_caption: Option<Caption>,
}

impl IdentifiableCommand for UpdateCaptionCommand {
    type Id = PostId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<PostId> {
        &self.target
    }
}

impl UpdateCaptionCommand {
    pub fn try_from_proto(req: UpdateCaptionRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target metadata"))?;

        let new_caption = if req.new_caption.is_empty() {
            None
        } else {
            Some(Caption::try_new(req.new_caption)?)
        };

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|_| Error::validation("command_id", "Invalid UUID"))?,
            target: CommandTarget {
                id: PostId::try_from(proto_target.post_id)?,
                region: Region::try_new(proto_target.region)?,
                expected_version: None,
            },
            new_caption,
        })
    }
}
