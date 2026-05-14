// crates/profile/src/application/commands/media/remove_banner/remove_banner_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::RemoveBannerCommand, context::ProfileContext};

pub struct RemoveBannerHandler;

#[async_trait]
impl CommandHandler for RemoveBannerHandler {
    type Context = ProfileContext;
    type Command = RemoveBannerCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: RemoveBannerCommand) -> Result<Self::Output> {
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
