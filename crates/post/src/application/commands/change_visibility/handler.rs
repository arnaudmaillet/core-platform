// crates/post/src/application/handlers/update_visibility_handler.rs

use crate::application::{commands::ChangeVisibilityCommand, context::PostCommandCtx};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct ChangeVisibilityHandler;

#[async_trait]
impl CommandHandler for ChangeVisibilityHandler {
    type Context = PostCommandCtx;
    type Command = ChangeVisibilityCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandCtx,
        cmd: ChangeVisibilityCommand,
    ) -> Result<Self::Output> {
        let mut post = ctx.fetch_verified(&cmd.target).await?;
        if post.change_visibility(cmd.new_visibility)? {
            ctx.save(&mut post, cmd.command_id).await?;
        } else {
            info!(
                post_id = %post.post_id(),
                "visibility is already set to {:?}, skipping save",
                cmd.new_visibility
            );
        }

        Ok(())
    }
}
