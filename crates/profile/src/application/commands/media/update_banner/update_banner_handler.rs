// crates/profile/src/application/commands/media/update_banner/update_banner_handler.rs

use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateBannerCommand, context::ProfileContext};

pub struct UpdateBannerHandler;

#[async_trait]
impl CommandHandler for UpdateBannerHandler {
    type Context = ProfileContext;
    type Command = UpdateBannerCommand;
    type Output = ();

    async fn handle(&self, ctx: &ProfileContext, cmd: UpdateBannerCommand) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_banner(cmd.new_banner_url)? {
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
