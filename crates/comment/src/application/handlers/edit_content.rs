// crates/content_comments/src/application/handlers/edit_comment_content_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::{commands::EditCommentContentCommand, context::CommentCommandContext};

pub struct EditCommentContentHandler;

#[async_trait]
impl CommandHandler for EditCommentContentHandler {
    type Context = CommentCommandContext;
    type Command = EditCommentContentCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &CommentCommandContext,
        cmd: EditCommentContentCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id).await? {
            return Ok(());
        }

        let mut comment = ctx
            .fetch_verified(&cmd.target, cmd.post_id, cmd.parent_comment_id)
            .await?;

        if comment.edit_content(cmd.editor_id, cmd.new_content)? {
            ctx.save(&mut comment, Some(cmd.command_id)).await?;
            info!(comment_id = %comment.comment_id(), "Comment content updated in ScyllaDB");
        } else {
            info!(comment_id = %comment.comment_id(), "Content identical, skipping write");
            ctx.save_idempotency(cmd.command_id).await?;
        }

        Ok(())
    }
}
