// crates/post/src/application/handlers/delete_post_handler.rs

use crate::application::{commands::DeletePostCommand, context::PostCommandContext};
use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};

pub struct DeletePostHandler;

#[async_trait]
impl CommandHandler for DeletePostHandler {
    type Context = PostCommandContext;
    type Command = DeletePostCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &PostCommandContext,
        cmd: DeletePostCommand,
    ) -> Result<Self::Output> {
        if !ctx.ensure_executable(cmd.command_id, cmd.region).await? {
            return Ok(());
        }

        let post = ctx.fetch_verified(&cmd.target).await?;

        ctx.delete(&post, cmd.command_id).await?;

        Ok(())
    }
}
