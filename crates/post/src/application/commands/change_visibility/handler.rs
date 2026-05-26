// crates/post/src/application/handlers/update_visibility_handler.rs

use crate::application::{commands::ChangeVisibilityCommand, context::PostCommandContext};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

pub struct ChangeVisibilityHandler;

#[async_trait]
impl CommandHandler for ChangeVisibilityHandler {
    type Context = PostCommandContext;
    type Command = ChangeVisibilityCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandContext,
        cmd: ChangeVisibilityCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut post = ctx.fetch_verified(&cmd.target).await?;

        if post.change_visibility(cmd.new_visibility)? {
            ctx.save(&mut post, Some(cmd.command_id)).await?;
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
