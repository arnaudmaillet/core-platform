// crates/profile/src/application/commands/media/remove_banner/remove_banner_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::RemoveBannerCommand, context::ProfileCommandContext};

pub struct RemoveBannerHandler;

#[async_trait]
impl CommandHandler for RemoveBannerHandler {
    type Context = ProfileCommandContext;
    type Command = RemoveBannerCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileCommandContext, cmd: RemoveBannerCommand) -> Result<Self::Output> {
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.remove_banner()? {
            ctx.save(&mut profile, Some(cmd.command_id)).await?;
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "no changes detected, skipping save"
            );
        }
        Ok(())
    }
}
