// crates/content_comments/src/application/commands/publish_comment.rs

use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, ProfileId};
use shared_proto::comment::v1::PublishCommentRequest;
use uuid::Uuid;

use crate::types::{CommentContent, CommentId};

#[derive(Debug, Deserialize, Clone)]
pub struct PublishCommentCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<PostId>,
    pub profile_id: ProfileId,
    pub parent_comment_id: Option<CommentId>,
    pub content: CommentContent,
}

impl IdentifiableCommand for PublishCommentCommand {
    type Id = PostId;
    type Routing = ();

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<PostId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        ()
    }
}

impl PublishCommentCommand {
    pub fn try_from_proto(req: PublishCommentRequest) -> Result<Self> {
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let post_id = PostId::try_from(req.post_id)?;

        let parent_comment_id = if let Some(parent_str) = req.parent_comment_id {
            if parent_str.is_empty() {
                None
            } else {
                let parent_uuid = Uuid::parse_str(&parent_str).map_err(|_| {
                    Error::validation("parent_comment_id", "Invalid parent UUID format")
                })?;
                Some(CommentId::from(parent_uuid))
            }
        } else {
            None
        };

        let content = CommentContent::try_new(req.content)?;

        Ok(Self {
            command_id,
            target: CommandTarget {
                id: post_id,
                expected_version: None,
            },
            profile_id: ProfileId::try_from(req.profile_id)?,
            parent_comment_id,
            content,
        })
    }
}
