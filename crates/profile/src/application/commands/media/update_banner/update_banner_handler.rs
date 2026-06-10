// crates/profile/src/application/commands/media/update_banner/update_banner_handler.rs

use async_trait::async_trait;
use shared_kernel::{command::CommandHandler, core::Result};
use tracing::info;

use crate::{commands::UpdateBannerCommand, context::ProfileCommandContext};

pub struct UpdateBannerHandler;

impl UpdateBannerHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for UpdateBannerHandler {
    type Context = ProfileCommandContext;
    type Command = UpdateBannerCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: UpdateBannerCommand,
    ) -> Result<Self::Output> {
        let mut profile = ctx.fetch_verified(&cmd.target).await?;

        if profile.update_banner(cmd.new_banner_url)? {
            ctx.save(&mut profile).await?;

            info!(
                profile_id = %profile.profile_id(),
                "Banner updated successfully"
            );
        } else {
            info!(
                profile_id = %profile.profile_id(),
                "Banner URL is already the same, skipping save"
            );
        }

        Ok(())
    }
}
