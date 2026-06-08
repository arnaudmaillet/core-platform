// crates/content_comments/src/application/handlers/delete_comment_handler.rs

use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

use crate::application::{commands::DeleteCommentCommand, context::CommentCommandContext};

pub struct DeleteCommentHandler;

#[async_trait]
impl CommandHandler for DeleteCommentHandler {
    type Context = CommentCommandContext;
    type Command = DeleteCommentCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &CommentCommandContext,
        cmd: DeleteCommentCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id).await? {
            return Ok(());
        }

        let mut comment = ctx
            .fetch_verified(&cmd.target, cmd.post_id, cmd.parent_comment_id)
            .await?;

        if comment.delete_content(cmd.operator_id)? {
            ctx.save(&mut comment, Some(cmd.command_id)).await?;
            info!(
                comment_id = %comment.comment_id(),
                operator_id = %cmd.operator_id,
                "Comment soft-deleted successfully"
            );
        } else {
            info!(comment_id = %comment.comment_id(), "Comment already deleted, skipping write");
            ctx.save_idempotency(cmd.command_id).await?;
        }

        Ok(())
    }
}
