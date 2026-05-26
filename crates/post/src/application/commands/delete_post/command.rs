// crates/post/src/application/commands/delete_post.rs

use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, Region};
use shared_proto::post::v1::DeletePostRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct DeletePostCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<PostId>,
}

impl IdentifiableCommand for DeletePostCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }
    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }
    fn region(&self) -> String {
        self.target.region.to_string()
    }
    fn cache_key(&self) -> Option<String> {
        Some(format!("posts:{}:{}", self.target.region, self.target.id))
    }
}

impl DeletePostCommand {
    pub fn try_from_proto(req: DeletePostRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing target"))?;

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|_| Error::validation("command_id", "Invalid UUID"))?,
            target: CommandTarget {
                id: PostId::try_from(proto_target.post_id)?,
                region: Region::try_new(proto_target.region)?,
                expected_version: proto_target.expected_version,
            },
        })
    }
}
