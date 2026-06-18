// crates/post/src/application/handlers/toggle_comments_handler.rs

use crate::post::application::{context::PostCommandCtx, handlers::ToggleCommentsCommand};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct ToggleCommentsHandler;

#[async_trait]
impl CommandHandler for ToggleCommentsHandler {
    type Context = PostCommandCtx;
    type Command = ToggleCommentsCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandCtx,
        cmd: ToggleCommentsCommand,
    ) -> Result<Self::Output> {
        let mut post = ctx.fetch_verified(&cmd.target).await?;
        if post.toggle_comments(cmd.allowed)? {
            ctx.save(&mut post, cmd.command_id).await?;
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
