// crates/content_comments/src/application/commands/edit_comment_content.rs

use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{PostId, ProfileId};
use shared_proto::comment::v1::EditCommentContentRequest;
use uuid::Uuid;

use crate::types::{CommentContent, CommentId};

#[derive(Debug, Deserialize, Clone)]
pub struct EditCommentContentCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<CommentId>,
    pub post_id: PostId,
    pub parent_comment_id: Option<CommentId>,
    pub editor_id: ProfileId,
    pub new_content: CommentContent,
}

impl IdentifiableCommand for EditCommentContentCommand {
    type Id = CommentId;
    type Routing = ();

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<CommentId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        ()
    }
}

impl EditCommentContentCommand {
    pub fn try_from_proto(req: EditCommentContentRequest) -> Result<Self> {
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing comment target"))?;

        let comment_uuid = Uuid::parse_str(&proto_target.comment_id)
            .map_err(|_| Error::validation("target.comment_id", "Invalid Comment UUID format"))?;

        let post_id = PostId::try_from(proto_target.post_id)?;

        let parent_comment_id = if let Some(parent_str) = proto_target.parent_comment_id {
            if parent_str.is_empty() {
                None
            } else {
                let parent_uuid = Uuid::parse_str(&parent_str).map_err(|_| {
                    Error::validation("target.parent_comment_id", "Invalid parent UUID format")
                })?;
                Some(CommentId::from(parent_uuid))
            }
        } else {
            None
        };

        let editor_id = ProfileId::try_from(req.operator_id)?;
        let new_content = CommentContent::try_new(req.new_content)?;

        Ok(Self {
            command_id,
            target: CommandTarget {
                id: CommentId::from(comment_uuid),
                expected_version: None,
            },
            post_id,
            parent_comment_id,
            editor_id,
            new_content,
        })
    }
}
