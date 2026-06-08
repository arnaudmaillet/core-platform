// crates/post/src/application/handlers/toggle_comments_handler.rs

use crate::application::{commands::ToggleCommentsCommand, context::PostCommandContext};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct ToggleCommentsHandler;

#[async_trait]
impl CommandHandler for ToggleCommentsHandler {
    type Context = PostCommandContext;
    type Command = ToggleCommentsCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandContext,
        cmd: ToggleCommentsCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.region)
            .await?
        {
            return Ok(());
        }

        let mut post = ctx.fetch_verified(&cmd.target).await?;

        if post.toggle_comments(cmd.allowed)? {
            ctx.save(&mut post, Some(cmd.command_id)).await?;
        } else {
            info!(
                post_id = %post.post_id(),
                current = post.allowed_comment_hands(),
                "comment status is already set to {}, skipping save",
                cmd.allowed
            );
        }

        Ok(())
    }
}
