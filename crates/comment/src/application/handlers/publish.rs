// crates/content_comments/src/application/handlers/publish_comment_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Error, Result};

use crate::application::{commands::PublishCommentCommand, context::CommentCommandContext};
use crate::entities::Comment;
use crate::types::CommentId;

pub struct PublishCommentHandler;

#[async_trait]
impl CommandHandler for PublishCommentHandler {
    type Context = CommentCommandContext;
    type Command = PublishCommentCommand;
    type Output = CommentId;

    async fn handle(
        &self,
        ctx: &CommentCommandContext,
        cmd: PublishCommentCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id).await? {
            return Err(Error::already_exists(
                "Command",
                "id",
                cmd.command_id.to_string(),
            ));
        }

        let mut comment = Comment::builder(cmd.target.id, ctx.operator_id(), cmd.content)?
            .with_parent_comment_id(cmd.parent_comment_id)
            .build()?;

        comment.publish_comment()?;
        ctx.save(&mut comment, Some(cmd.command_id)).await?;

        Ok(comment.comment_id())
    }
}
